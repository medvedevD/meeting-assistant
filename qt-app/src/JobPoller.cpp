#include "JobPoller.h"

#include "ApiClient.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <QVariantMap>

JobPoller::JobPoller(QObject *parent) : QObject(parent) {
    m_timer = new QTimer(this);
    connect(m_timer, &QTimer::timeout, this, &JobPoller::tick);
}

void JobPoller::setApi(ApiClient *api) {
    if (m_api == api)
        return;
    if (m_api)
        m_api->disconnect(this);
    m_api = api;
    if (m_api) {
        connect(m_api, &ApiClient::requestSucceeded, this,
                &JobPoller::onSucceeded);
        connect(m_api, &ApiClient::requestFailed, this, &JobPoller::onFailed);
    }
    emit apiChanged();
}

void JobPoller::setJobId(const QString &id) {
    if (m_jobId == id)
        return;
    m_jobId = id;
    emit jobIdChanged();
}

void JobPoller::setIntervalMs(int ms) {
    if (m_intervalMs == ms || ms <= 0)
        return;
    m_intervalMs = ms;
    m_timer->setInterval(ms);
    emit intervalMsChanged();
}

void JobPoller::setActive(bool a) {
    if (m_active == a)
        return;
    m_active = a;
    emit activeChanged();
}

bool JobPoller::isTerminal(const QString &status) {
    return status == QStringLiteral("done") ||
           status == QStringLiteral("failed");
}

void JobPoller::start() {
    if (!m_api || m_jobId.isEmpty() || m_active)
        return;
    setActive(true);
    // Prefer the live SSE stream; fall back to polling if it can't be opened.
    if (!startStream())
        startPolling();
}

void JobPoller::stop() {
    m_timer->stop();
    m_inFlightId = 0;
    teardownStream();
    setActive(false);
}

// ── shared update sink ───────────────────────────────────────────────────────

void JobPoller::applyUpdate(const QString &status, const QVariant &json) {
    if (status != m_status) {
        m_status = status;
        emit statusChanged(status, json);
    }
    // Always push the latest snapshot so progress (percent) updates live, not
    // only on status transitions.
    emit jobUpdated(status, json);
    if (isTerminal(status))
        stop();
}

// ── polling transport (fallback) ─────────────────────────────────────────────

void JobPoller::startPolling() {
    m_timer->start(m_intervalMs);
    tick(); // poll immediately, then on each interval
}

void JobPoller::tick() {
    if (!m_api || m_jobId.isEmpty())
        return;
    if (m_inFlightId != 0)
        return; // don't stack requests if the core is slow
    m_inFlightId = m_api->get(QStringLiteral("/api/v1/jobs/") + m_jobId);
}

void JobPoller::onSucceeded(int requestId, const QString & /*path*/,
                            const QVariant &json) {
    if (requestId != m_inFlightId)
        return;
    m_inFlightId = 0;

    const QVariantMap job = json.toMap();
    const QString status = job.value(QStringLiteral("status")).toString();
    applyUpdate(status, json);
}

void JobPoller::onFailed(int requestId, const QString & /*path*/,
                         int /*httpStatus*/, const QString &error) {
    if (requestId != m_inFlightId)
        return;
    m_inFlightId = 0;
    emit failed(error); // keep polling — a transient error shouldn't end tracking
}

// ── SSE transport (preferred) ────────────────────────────────────────────────

bool JobPoller::startStream() {
    if (!m_api || !m_api->isConfigured())
        return false;
    QNetworkReply *reply = m_api->streamGet(QStringLiteral("/api/v1/jobs/") +
                                            m_jobId + QStringLiteral("/events"));
    if (!reply)
        return false;

    m_sse = reply;
    m_sseBuf.clear();
    connect(m_sse, &QIODevice::readyRead, this, &JobPoller::onStreamReadyRead);
    connect(m_sse, &QNetworkReply::finished, this, &JobPoller::onStreamFinished);

    // Guard the initial connect: if no byte arrives before the watchdog fires,
    // the stream is treated as failed and we fall back to polling.
    if (!m_streamWatchdog) {
        m_streamWatchdog = new QTimer(this);
        m_streamWatchdog->setSingleShot(true);
        connect(m_streamWatchdog, &QTimer::timeout, this,
                &JobPoller::onStreamTimeout);
    }
    m_streamWatchdog->start(kStreamConnectTimeoutMs);
    return true;
}

void JobPoller::onStreamReadyRead() {
    if (!m_sse)
        return;
    if (m_streamWatchdog)
        m_streamWatchdog->stop(); // first bytes → the stream connected

    // A non-2xx response (e.g. 404 once the job is gone, 401) is not an event
    // stream. Drain it and let finished() drive the polling fallback.
    const int http =
        m_sse->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    if (http != 0 && (http < 200 || http >= 300)) {
        m_sse->readAll();
        return;
    }

    m_sseBuf.append(m_sse->readAll());
    processStreamBuffer();
}

void JobPoller::onStreamFinished() {
    if (!m_sse)
        return;
    if (m_streamWatchdog)
        m_streamWatchdog->stop();

    // Flush any trailing complete frames the socket delivered alongside EOF.
    m_sseBuf.append(m_sse->readAll());
    processStreamBuffer();
    if (!m_active)
        return; // a terminal frame already stopped us

    // The stream ended without a terminal status (network cut, server close, or
    // a non-2xx response). Fall back to polling so tracking continues. This is
    // not a job failure, so we do NOT emit failed().
    teardownStream();
    if (m_active)
        startPolling();
}

void JobPoller::onStreamTimeout() {
    if (!m_active)
        return;
    // No byte arrived within the connect budget — abandon the stream and poll.
    teardownStream();
    startPolling();
}

void JobPoller::processStreamBuffer() {
    int sep;
    while ((sep = m_sseBuf.indexOf("\n\n")) >= 0) {
        const QByteArray frame = m_sseBuf.left(sep);
        m_sseBuf.remove(0, sep + 2);

        QString event;
        QVariant json;
        if (!decodeSseFrame(frame, event, json))
            continue; // comment/keepalive or unparseable

        const QString status =
            json.toMap().value(QStringLiteral("status")).toString();
        applyUpdate(status, json);
        if (!m_active)
            return; // terminal handled — stream torn down, buffer cleared
    }
}

void JobPoller::teardownStream() {
    if (m_streamWatchdog)
        m_streamWatchdog->stop();
    if (m_sse) {
        m_sse->disconnect(this); // silence finished() from the abort below
        m_sse->abort();
        m_sse->deleteLater();
        m_sse = nullptr;
    }
    m_sseBuf.clear();
}

bool JobPoller::decodeSseFrame(const QByteArray &frame, QString &eventOut,
                               QVariant &jsonOut) {
    eventOut.clear();
    QByteArray data;
    const QList<QByteArray> lines = frame.split('\n');
    for (QByteArray line : lines) {
        if (line.endsWith('\r'))
            line.chop(1);
        if (line.isEmpty() || line.startsWith(':'))
            continue; // blank line or comment (keepalive)
        if (line.startsWith("event:")) {
            eventOut = QString::fromUtf8(line.mid(6).trimmed());
        } else if (line.startsWith("data:")) {
            if (!data.isEmpty())
                data.append('\n');
            data.append(line.mid(5).trimmed());
        }
    }
    if (data.isEmpty())
        return false;

    QJsonParseError perr;
    const QJsonDocument doc = QJsonDocument::fromJson(data, &perr);
    if (perr.error != QJsonParseError::NoError)
        return false;
    jsonOut = doc.toVariant();
    return true;
}
