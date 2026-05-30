#include "ApiClient.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QUrl>

ApiClient::ApiClient(QObject *parent) : QObject(parent) {
    m_net = new QNetworkAccessManager(this);
}

void ApiClient::configure(const QString &baseUrl, const QString &token) {
    m_baseUrl = baseUrl;
    m_bearer = QByteArrayLiteral("Bearer ") + token.toUtf8();
    m_configured = !baseUrl.isEmpty() && !token.isEmpty();
    emit configuredChanged();
}

int ApiClient::get(const QString &path) {
    return send("GET", path, {}, /*hasBody=*/false);
}

int ApiClient::post(const QString &path, const QVariantMap &body) {
    return send("POST", path, body, /*hasBody=*/true);
}

int ApiClient::put(const QString &path, const QVariantMap &body) {
    return send("PUT", path, body, /*hasBody=*/true);
}

int ApiClient::del(const QString &path) {
    return send("DELETE", path, {}, /*hasBody=*/false);
}

int ApiClient::send(const QByteArray &verb, const QString &path,
                     const QVariantMap &body, bool hasBody) {
    const int id = m_nextId++;

    if (!m_configured) {
        // Defer the failure to the event loop so callers always observe it via
        // the signal, never synchronously mid-call.
        QMetaObject::invokeMethod(
            this,
            [this, id, path]() {
                emit requestFailed(id, path, 0,
                                   QStringLiteral("ApiClient not configured "
                                                  "(sidecar not ready)"));
            },
            Qt::QueuedConnection);
        return id;
    }

    QNetworkRequest req{QUrl(m_baseUrl + path)};
    req.setRawHeader(QByteArrayLiteral("Authorization"), m_bearer);
    req.setHeader(QNetworkRequest::ContentTypeHeader,
                  QStringLiteral("application/json"));

    QNetworkReply *reply = nullptr;
    if (hasBody) {
        const QByteArray payload = QJsonDocument(QJsonObject::fromVariantMap(body))
                                       .toJson(QJsonDocument::Compact);
        reply = m_net->sendCustomRequest(req, verb, payload);
    } else {
        reply = m_net->sendCustomRequest(req, verb);
    }

    connect(reply, &QNetworkReply::finished, this, [this, reply, id, path]() {
        reply->deleteLater();
        const int httpStatus =
            reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
        const QByteArray raw = reply->readAll();

        if (reply->error() != QNetworkReply::NoError || httpStatus >= 400) {
            QString msg = reply->errorString();
            if (!raw.isEmpty())
                msg += QStringLiteral(" — ") + QString::fromUtf8(raw);
            emit requestFailed(id, path, httpStatus, msg);
            return;
        }

        QJsonParseError perr;
        const QJsonDocument doc = QJsonDocument::fromJson(raw, &perr);
        if (!raw.isEmpty() && perr.error != QJsonParseError::NoError) {
            emit requestFailed(id, path, httpStatus,
                               QStringLiteral("invalid JSON: %1")
                                   .arg(perr.errorString()));
            return;
        }
        emit requestSucceeded(id, path, doc.toVariant());
    });

    return id;
}
