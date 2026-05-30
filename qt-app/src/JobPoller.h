#pragma once

#include <QObject>
#include <QString>
#include <QVariant>

class ApiClient;
class QTimer;

// Polls `GET /api/v1/jobs/:id` on a QTimer and emits `jobUpdated` on each poll.
// A QML screen sets `api` + `jobId`, calls start(), and reacts to `jobUpdated`
// (status + decoded job); polling auto-stops on a terminal status. Registered
// as a creatable QML type so a screen can own one poller per tracked job.
class JobPoller : public QObject {
    Q_OBJECT
    Q_PROPERTY(ApiClient *api READ api WRITE setApi NOTIFY apiChanged)
    Q_PROPERTY(QString jobId READ jobId WRITE setJobId NOTIFY jobIdChanged)
    Q_PROPERTY(int intervalMs READ intervalMs WRITE setIntervalMs NOTIFY
                   intervalMsChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)
    Q_PROPERTY(bool active READ isActive NOTIFY activeChanged)

public:
    explicit JobPoller(QObject *parent = nullptr);

    ApiClient *api() const { return m_api; }
    void setApi(ApiClient *api);

    QString jobId() const { return m_jobId; }
    void setJobId(const QString &id);

    int intervalMs() const { return m_intervalMs; }
    void setIntervalMs(int ms);

    QString status() const { return m_status; }
    bool isActive() const { return m_active; }

    Q_INVOKABLE void start();
    Q_INVOKABLE void stop();

signals:
    void apiChanged();
    void jobIdChanged();
    void intervalMsChanged();
    // `job` is the full decoded JobResponse object. Kept with the original
    // payload for QML compatibility; `jobUpdated` is emitted on every poll.
    void statusChanged(const QString &status, const QVariant &job);
    // Emitted on every successful poll with the full decoded JobResponse, so a
    // QML consumer gets both live progress and the terminal status/`job`.
    void jobUpdated(const QString &status, const QVariant &job);
    void activeChanged();
    void failed(const QString &error);

private slots:
    void tick();
    void onSucceeded(int requestId, const QString &path, const QVariant &json);
    void onFailed(int requestId, const QString &path, int httpStatus,
                  const QString &error);

private:
    void setActive(bool a);
    static bool isTerminal(const QString &status);

    ApiClient *m_api = nullptr;
    QTimer *m_timer = nullptr;
    QString m_jobId;
    QString m_status;
    int m_intervalMs = 1000;
    int m_inFlightId = 0;
    bool m_active = false;
};
