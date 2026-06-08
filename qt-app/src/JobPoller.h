#pragma once

#include <QByteArray>
#include <QObject>
#include <QString>
#include <QVariant>

class ApiClient;
class QTimer;
class QNetworkReply;

// Tracks one async job's progress and emits `jobUpdated`/`statusChanged` on
// every update. A QML screen sets `api` + `jobId`, calls start(), and reacts
// to those signals (status + decoded job); tracking auto-stops on a terminal
// status. Registered as a creatable QML type so a screen can own one poller
// per tracked job.
//
// Transport: stream-first with a polling fallback. start() opens an SSE stream
// (`GET /api/v1/jobs/:id/events`) for near-real-time updates; if the stream
// cannot be opened, stalls before first byte, or drops mid-flight, it falls
// back to timer-based polling of `GET /api/v1/jobs/:id` so tracking always
// survives. The public contract (properties + signals) is identical either
// way, so QML consumers need no change.
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

public:
    // Decode one SSE frame (the bytes between blank-line separators) into its
    // event name and the JSON carried by its `data:` line(s). Returns false for
    // comment-only (keepalive) frames and for unparseable data. Static + pure
    // so the wire-parsing contract can be unit-tested without a live socket.
    static bool decodeSseFrame(const QByteArray &frame, QString &eventOut,
                               QVariant &jsonOut);

private slots:
    void tick();
    void onSucceeded(int requestId, const QString &path, const QVariant &json);
    void onFailed(int requestId, const QString &path, int httpStatus,
                  const QString &error);
    void onStreamReadyRead();
    void onStreamFinished();
    void onStreamTimeout();

private:
    void setActive(bool a);
    static bool isTerminal(const QString &status);

    // Shared sink for both transports: diff the status, emit the signals, and
    // stop on a terminal state.
    void applyUpdate(const QString &status, const QVariant &json);

    // Transport control.
    bool startStream();          // returns false if a stream can't be opened
    void startPolling();         // timer-based fallback
    void processStreamBuffer();  // parse + dispatch complete buffered frames
    void teardownStream();       // abort + free the SSE reply, reset stream state

    static constexpr int kStreamConnectTimeoutMs = 4000;

    ApiClient *m_api = nullptr;
    QTimer *m_timer = nullptr;
    QString m_jobId;
    QString m_status;
    int m_intervalMs = 1000;
    int m_inFlightId = 0;
    bool m_active = false;

    // SSE stream state (null/empty while polling).
    QNetworkReply *m_sse = nullptr;
    QByteArray m_sseBuf;
    QTimer *m_streamWatchdog = nullptr;
};
