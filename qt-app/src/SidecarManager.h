#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QtGlobal>

class QNetworkAccessManager;
class QTimer;

// Owns the `meeting-server` sidecar process and the client half of the
// loopback-HTTP contract:
//
//   spawn  ->  parse the one-line stdout handshake  ->  version gate (Q9)
//          ->  /health readiness gate  ->  Ready
//
// Parent-death linkage keeps the child from being orphaned if the GUI is
// killed (POSIX: inherited pipe whose write end the GUI holds + --parent-pid;
// Windows: a kill-on-close Job Object + --parent-pid). On a clean quit the GUI
// also actively terminate()->kill()s the child. An unexpected child exit while
// running drives a bounded respawn ("core restarting").
class SidecarManager : public QObject {
    Q_OBJECT
    Q_PROPERTY(State state READ state NOTIFY stateChanged)
    Q_PROPERTY(QString stateText READ stateText NOTIFY stateChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY stateChanged)
    Q_PROPERTY(QString baseUrl READ baseUrl NOTIFY readyChanged)
    Q_PROPERTY(quint32 clientProtocol READ clientProtocol CONSTANT)
    Q_PROPERTY(quint32 serverProtocol READ serverProtocol NOTIFY handshakeChanged)
    Q_PROPERTY(quint32 serverMinProtocol READ serverMinProtocol NOTIFY handshakeChanged)
    Q_PROPERTY(QString serverBuild READ serverBuild NOTIFY handshakeChanged)

public:
    enum State {
        Idle,         // not started yet
        Starting,     // spawning / handshake / health gate
        Ready,        // handshake ok, version ok, /health ok
        Restarting,   // child died unexpectedly; respawning
        Incompatible, // protocol-range mismatch — terminal, do not proceed
        Failed        // fatal (singleton busy / restart budget exhausted)
    };
    Q_ENUM(State)

    explicit SidecarManager(QObject *parent = nullptr);
    ~SidecarManager() override;

    State state() const { return m_state; }
    QString stateText() const;
    QString lastError() const { return m_lastError; }
    QString baseUrl() const { return m_baseUrl; }
    quint32 clientProtocol() const { return m_clientProtocol; }
    quint32 serverProtocol() const { return m_serverProtocol; }
    quint32 serverMinProtocol() const { return m_serverMinProtocol; }
    QString serverBuild() const { return m_serverBuild; }

    // Begin the spawn/handshake/health sequence (idempotent if already live).
    void start();

    // Terminate the child cleanly: terminate() -> wait -> kill() -> wait, and
    // drop the parent-liveness pipe so a missed signal still triggers child
    // exit. Safe to call repeatedly; called from QGuiApplication::aboutToQuit.
    Q_INVOKABLE void shutdown();

signals:
    void stateChanged();
    void readyChanged();
    void handshakeChanged();
    // Emitted once the sidecar is fully Ready — wires the ApiClient.
    void ready(const QString &baseUrl, const QString &token);
    // Emitted when the version gate fails. The UI must block and not proceed.
    void versionMismatch(quint32 clientProtocol, quint32 serverMin, quint32 serverMax);

private slots:
    void onStdoutReadyRead();
    void onStderrReadyRead();
    void onProcessErrorOccurred(QProcess::ProcessError error);
    void onProcessFinished(int exitCode, QProcess::ExitStatus status);
    void pollHealth();

private:
    void spawnProcess();
    void setState(State s, const QString &error = QString());
    void handleHandshakeLine(const QByteArray &line);
    bool versionGate(); // false => Incompatible (already transitioned)
    void beginHealthGate();
    void scheduleRespawn();
    QString locateSidecar() const;

    // Platform parent-death linkage.
    void setupParentDeathLinkage(QProcess &proc, QStringList &args);
    void releaseParentDeathLinkage();
    void assignToJobObjectIfWindows();

    QProcess m_proc;
    QNetworkAccessManager *m_net = nullptr;
    QTimer *m_healthTimer = nullptr;

    State m_state = Idle;
    QString m_lastError;
    QString m_baseUrl;
    QString m_token;
    quint16 m_port = 0;
    quint32 m_clientProtocol = 0;
    quint32 m_serverProtocol = 0;
    quint32 m_serverMinProtocol = 0;
    QString m_serverBuild;

    QByteArray m_stdoutBuf;
    bool m_handshakeSeen = false;
    bool m_shuttingDown = false;
    int m_restartCount = 0;
    int m_healthAttempts = 0;

    static constexpr int kMaxRestarts = 5;
    static constexpr int kHealthMaxAttempts = 15; // ~15 * 200ms = 3s
    static constexpr int kHealthIntervalMs = 200;

#ifdef Q_OS_UNIX
    int m_parentPipeWriteFd = -1; // GUI holds this; child reads EOF on our death
    int m_parentPipeReadFd = -1;  // handed to the child via dup
#endif
#ifdef Q_OS_WIN
    void *m_jobHandle = nullptr; // HANDLE; kill-on-close ties child to our life
#endif
};
