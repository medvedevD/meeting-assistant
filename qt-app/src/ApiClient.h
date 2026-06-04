#pragma once

#include <QObject>
#include <QString>
#include <QVariant>

class QNetworkAccessManager;
class QNetworkReply;

// Thin authenticated HTTP client over the loopback sidecar, exposed to QML.
//
// Wraps QNetworkAccessManager: base URL `http://127.0.0.1:<port>`, injects
// `Authorization: Bearer <token>` on every request, JSON in/out via
// QJsonDocument. Async — each call returns an opaque request id and later emits
// requestSucceeded / requestFailed carrying that id so QML can correlate.
class ApiClient : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool configured READ isConfigured NOTIFY configuredChanged)
    Q_PROPERTY(QString baseUrl READ baseUrl NOTIFY configuredChanged)

public:
    explicit ApiClient(QObject *parent = nullptr);

    bool isConfigured() const { return m_configured; }
    QString baseUrl() const { return m_baseUrl; }

    // Wired from SidecarManager::ready once the handshake + health gate pass.
    void configure(const QString &baseUrl, const QString &token);

    // Generic verbs. `path` is relative (e.g. "/api/v1/meetings").
    // `body` (POST) is serialised as a JSON object. Returns a request id.
    Q_INVOKABLE int get(const QString &path);
    Q_INVOKABLE int post(const QString &path, const QVariantMap &body);
    Q_INVOKABLE int put(const QString &path, const QVariantMap &body);
    // DELETE — `del` because `delete` is a C++ keyword. No body.
    Q_INVOKABLE int del(const QString &path);

    // Open an authenticated streaming GET (e.g. an SSE `text/event-stream`
    // endpoint). Unlike the verbs above, the reply is returned to the caller
    // with NO finished-handler attached and NO auto-read: the caller owns the
    // reply, drives it incrementally via readyRead, and deletes it. Returns
    // nullptr when the client is not yet configured. C++-only (used by
    // JobPoller); the bearer token never leaves ApiClient.
    QNetworkReply *streamGet(const QString &path);

signals:
    void configuredChanged();
    // `json` is the decoded response body (QVariantMap / QVariantList).
    void requestSucceeded(int requestId, const QString &path,
                          const QVariant &json);
    void requestFailed(int requestId, const QString &path, int httpStatus,
                       const QString &error);

private:
    int send(const QByteArray &verb, const QString &path,
             const QVariantMap &body, bool hasBody);

    QNetworkAccessManager *m_net = nullptr;
    QString m_baseUrl;
    QByteArray m_bearer;
    bool m_configured = false;
    int m_nextId = 1;
};
