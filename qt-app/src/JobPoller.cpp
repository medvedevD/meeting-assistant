#include "JobPoller.h"

#include "ApiClient.h"

#include <QJsonObject>
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
    m_timer->start(m_intervalMs);
    tick(); // poll immediately, then on each interval
}

void JobPoller::stop() {
    m_timer->stop();
    m_inFlightId = 0;
    setActive(false);
}

void JobPoller::tick() {
    if (!m_api || m_jobId.isEmpty())
        return;
    if (m_inFlightId != 0)
        return; // don't stack requests if the core is slow
    m_inFlightId =
        m_api->get(QStringLiteral("/api/v1/jobs/") + m_jobId);
}

void JobPoller::onSucceeded(int requestId, const QString & /*path*/,
                            const QVariant &json) {
    if (requestId != m_inFlightId)
        return;
    m_inFlightId = 0;

    const QVariantMap job = json.toMap();
    const QString status = job.value(QStringLiteral("status")).toString();
    if (status != m_status) {
        m_status = status;
        emit statusChanged(status, json);
    }
    if (isTerminal(status))
        stop();
}

void JobPoller::onFailed(int requestId, const QString & /*path*/,
                         int /*httpStatus*/, const QString &error) {
    if (requestId != m_inFlightId)
        return;
    m_inFlightId = 0;
    emit failed(error); // keep polling — a transient error shouldn't end tracking
}
