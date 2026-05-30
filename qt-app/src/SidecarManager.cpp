#include "SidecarManager.h"

#include "ClientProtocol.h" // generated: meetingassistant::kClientProtocol

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <QUrl>

#ifdef Q_OS_UNIX
#include <fcntl.h>
#include <unistd.h>
#endif

#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {
// `meeting-server` exits with this code when another instance already holds
// the singleton lock (rust/crates/app/src/bin/meeting-server.rs EXIT_SINGLETON_BUSY).
constexpr int kExitSingletonBusy = 3;
} // namespace

SidecarManager::SidecarManager(QObject *parent) : QObject(parent) {
    m_net = new QNetworkAccessManager(this);

    m_healthTimer = new QTimer(this);
    m_healthTimer->setInterval(kHealthIntervalMs);
    connect(m_healthTimer, &QTimer::timeout, this, &SidecarManager::pollHealth);

    // The compiled-in client protocol comes from the Rust single source of
    // truth (codegen). MA_FORCE_CLIENT_PROTOCOL lets the version gate be
    // exercised end-to-end without rebuilding (acceptance: simulated mismatch).
    m_clientProtocol = meetingassistant::kClientProtocol;
    if (const QByteArray ov = qgetenv("MA_FORCE_CLIENT_PROTOCOL"); !ov.isEmpty()) {
        bool ok = false;
        const quint32 forced = ov.toUInt(&ok);
        if (ok) {
            qWarning("SidecarManager: MA_FORCE_CLIENT_PROTOCOL=%u overrides "
                     "kClientProtocol=%u (test hook)",
                     forced, m_clientProtocol);
            m_clientProtocol = forced;
        }
    }

    connect(&m_proc, &QProcess::readyReadStandardOutput, this,
            &SidecarManager::onStdoutReadyRead);
    connect(&m_proc, &QProcess::readyReadStandardError, this,
            &SidecarManager::onStderrReadyRead);
    connect(&m_proc, &QProcess::errorOccurred, this,
            &SidecarManager::onProcessErrorOccurred);
    connect(&m_proc, &QProcess::finished, this,
            &SidecarManager::onProcessFinished);
}

SidecarManager::~SidecarManager() {
    shutdown();
#ifdef Q_OS_WIN
    if (m_jobHandle) {
        // Closing the job kills the child (KILL_ON_JOB_CLOSE) as a last resort.
        ::CloseHandle(static_cast<HANDLE>(m_jobHandle));
        m_jobHandle = nullptr;
    }
#endif
}

QString SidecarManager::stateText() const {
    switch (m_state) {
    case Idle: return QStringLiteral("Idle");
    case Starting: return QStringLiteral("Starting core…");
    case Ready: return QStringLiteral("Ready");
    case Restarting: return QStringLiteral("Core restarting…");
    case Incompatible:
        return QStringLiteral("Components are incompatible — please update");
    case Failed: return QStringLiteral("Core failed to start");
    }
    return {};
}

void SidecarManager::setState(State s, const QString &error) {
    const bool changed = (m_state != s) || (m_lastError != error);
    m_state = s;
    m_lastError = error;
    if (changed) {
        qInfo("SidecarManager: state=%s%s", qPrintable(stateText()),
              error.isEmpty() ? "" : qPrintable(QStringLiteral(" (%1)").arg(error)));
        emit stateChanged();
    }
}

// ── Spawn ───────────────────────────────────────────────────────────────────

void SidecarManager::start() {
    if (m_state == Starting || m_state == Ready || m_state == Restarting)
        return;
    if (m_state == Incompatible || m_state == Failed)
        return; // terminal — start() does not retry a hard failure
    m_restartCount = 0;
    setState(Starting);
    spawnProcess();
}

QString SidecarManager::locateSidecar() const {
    // Dev/test override (run-qt.sh copies the binary next to the GUI; this
    // env lets a developer point at an arbitrary build).
    if (const QByteArray ov = qgetenv("MA_SIDECAR"); !ov.isEmpty())
        return QString::fromLocal8Bit(ov);

    QString name = QStringLiteral("meeting-server");
#ifdef Q_OS_WIN
    name += QStringLiteral(".exe");
#endif
    // The packaging layout (section-07) always places the sidecar in the same
    // directory as the GUI executable (macOS: Contents/MacOS, Linux: AppDir,
    // Windows: install dir).
    return QDir(QCoreApplication::applicationDirPath()).absoluteFilePath(name);
}

void SidecarManager::spawnProcess() {
    const QString path = locateSidecar();
    if (!QFileInfo::exists(path)) {
        setState(Failed,
                 QStringLiteral("meeting-server not found at %1").arg(path));
        return;
    }

    m_stdoutBuf.clear();
    m_handshakeSeen = false;

    QStringList args;
    // Backup parent-death mechanism on every OS (the pipe / Job Object is the
    // primary). The server polls this PID (~1 Hz) and also detects reparenting.
    args << QStringLiteral("--parent-pid")
         << QString::number(QCoreApplication::applicationPid());

    // May append --parent-pipe-fd and install the child-process modifier
    // (POSIX). Must run before setArguments so the fd arg is not dropped.
    setupParentDeathLinkage(m_proc, args);
    m_proc.setProgram(path);
    m_proc.setArguments(args);
    m_proc.start();

    if (!m_proc.waitForStarted(5000)) {
        setState(Failed, QStringLiteral("failed to launch %1: %2")
                              .arg(path, m_proc.errorString()));
        releaseParentDeathLinkage();
        return;
    }

    assignToJobObjectIfWindows();

#ifdef Q_OS_UNIX
    // Parent no longer needs the read end — only the child does. Keep the
    // write end (m_parentPipeWriteFd) open for the GUI's whole lifetime.
    if (m_parentPipeReadFd >= 0) {
        ::close(m_parentPipeReadFd);
        m_parentPipeReadFd = -1;
    }
#endif
}

// ── Handshake ───────────────────────────────────────────────────────────────

void SidecarManager::onStdoutReadyRead() {
    m_stdoutBuf += m_proc.readAllStandardOutput();
    if (m_handshakeSeen)
        return; // the contract: exactly ONE stdout line, ever
    const int nl = m_stdoutBuf.indexOf('\n');
    if (nl < 0)
        return; // wait for the full line
    const QByteArray line = m_stdoutBuf.left(nl);
    m_handshakeSeen = true;
    handleHandshakeLine(line);
}

void SidecarManager::onStderrReadyRead() {
    // Server logs go to stderr only — surface them on the GUI's debug stream.
    const QByteArray err = m_proc.readAllStandardError();
    for (const QByteArray &l : err.split('\n')) {
        if (!l.trimmed().isEmpty())
            qInfo("[meeting-server] %s", l.constData());
    }
}

void SidecarManager::handleHandshakeLine(const QByteArray &line) {
    QJsonParseError perr;
    const QJsonDocument doc = QJsonDocument::fromJson(line, &perr);
    if (perr.error != QJsonParseError::NoError || !doc.isObject()) {
        setState(Failed, QStringLiteral("invalid handshake from sidecar: %1")
                              .arg(perr.errorString()));
        shutdown();
        return;
    }
    const QJsonObject o = doc.object();
    if (!o.value(QStringLiteral("ready")).toBool(false)) {
        setState(Failed, QStringLiteral("sidecar reported not-ready"));
        shutdown();
        return;
    }

    m_port = static_cast<quint16>(o.value(QStringLiteral("port")).toInt());
    m_token = o.value(QStringLiteral("token")).toString();
    m_serverProtocol =
        static_cast<quint32>(o.value(QStringLiteral("protocol")).toInt());
    m_serverMinProtocol =
        static_cast<quint32>(o.value(QStringLiteral("min_protocol")).toInt());
    m_serverBuild = o.value(QStringLiteral("build")).toString();
    m_baseUrl = QStringLiteral("http://127.0.0.1:%1").arg(m_port);
    emit handshakeChanged();

    if (m_port == 0 || m_token.isEmpty()) {
        setState(Failed, QStringLiteral("handshake missing port/token"));
        shutdown();
        return;
    }

    // Q9 version gate — before any /api call or "Ready".
    if (!versionGate())
        return;

    beginHealthGate();
}

bool SidecarManager::versionGate() {
    const bool inRange = m_clientProtocol >= m_serverMinProtocol &&
                         m_clientProtocol <= m_serverProtocol;
    if (inRange)
        return true; // a build-only difference is fine — proceed

    qWarning("SidecarManager: protocol mismatch — client=%u, server range "
             "[%u, %u]; refusing to proceed",
             m_clientProtocol, m_serverMinProtocol, m_serverProtocol);
    setState(Incompatible);
    emit versionMismatch(m_clientProtocol, m_serverMinProtocol,
                         m_serverProtocol);
    shutdown(); // do not keep an unusable core alive
    return false;
}

// ── /health readiness gate ──────────────────────────────────────────────────

void SidecarManager::beginHealthGate() {
    m_healthAttempts = 0;
    m_healthTimer->start();
    pollHealth(); // probe immediately, then every kHealthIntervalMs
}

void SidecarManager::pollHealth() {
    if (m_state == Incompatible || m_state == Failed) {
        m_healthTimer->stop();
        return;
    }
    if (++m_healthAttempts > kHealthMaxAttempts) {
        m_healthTimer->stop();
        // Treat a never-healthy child as an unexpected failure → respawn path.
        qWarning("SidecarManager: /health never became ready");
        m_proc.kill();
        m_proc.waitForFinished(2000);
        return; // onProcessFinished drives the respawn/budget logic
    }

    QNetworkRequest req{QUrl(m_baseUrl + QStringLiteral("/health"))};
    QNetworkReply *reply = m_net->get(req);
    connect(reply, &QNetworkReply::finished, this, [this, reply]() {
        reply->deleteLater();
        if (m_state != Starting && m_state != Restarting)
            return;
        if (reply->error() != QNetworkReply::NoError)
            return; // not up yet — next tick retries
        const QJsonDocument d =
            QJsonDocument::fromJson(reply->readAll());
        if (d.isObject() &&
            d.object().value(QStringLiteral("status")).toString() ==
                QStringLiteral("ok")) {
            m_healthTimer->stop();
            m_restartCount = 0; // a clean ready resets the restart budget
            setState(Ready);
            emit readyChanged();
            emit ready(m_baseUrl, m_token);
        }
    });
}

// ── Process exit / failure handling ─────────────────────────────────────────

void SidecarManager::onProcessErrorOccurred(QProcess::ProcessError error) {
    if (error == QProcess::FailedToStart) {
        setState(Failed, QStringLiteral("failed to start meeting-server: %1")
                              .arg(m_proc.errorString()));
    }
}

void SidecarManager::onProcessFinished(int exitCode,
                                       QProcess::ExitStatus status) {
    m_healthTimer->stop();

    if (m_shuttingDown || m_state == Incompatible)
        return; // expected exit — nothing to recover

    if (exitCode == kExitSingletonBusy) {
        setState(Failed,
                 QStringLiteral("Another instance of Meeting Assistant is "
                                "already running."));
        return;
    }

    const QString why =
        status == QProcess::CrashExit
            ? QStringLiteral("core crashed")
            : QStringLiteral("core exited unexpectedly (code %1)").arg(exitCode);

    if (m_restartCount >= kMaxRestarts) {
        setState(Failed, QStringLiteral("%1 — restart budget exhausted "
                                        "(%2 attempts)")
                             .arg(why)
                             .arg(kMaxRestarts));
        return;
    }

    ++m_restartCount;
    setState(Restarting, why);
    scheduleRespawn();
}

void SidecarManager::scheduleRespawn() {
    releaseParentDeathLinkage(); // fresh pipe for the new child
    // Small backoff so a tight crash loop does not peg the CPU.
    QTimer::singleShot(500, this, [this]() {
        if (m_state == Restarting)
            spawnProcess();
    });
}

// ── Shutdown ────────────────────────────────────────────────────────────────

void SidecarManager::shutdown() {
    m_shuttingDown = true;
    m_healthTimer->stop();

    if (m_proc.state() != QProcess::NotRunning) {
        m_proc.terminate(); // SIGTERM / WM_CLOSE → graceful path in the server
        if (!m_proc.waitForFinished(3000)) {
            m_proc.kill(); // SIGKILL — guaranteed
            m_proc.waitForFinished(2000);
        }
    }

    // Dropping the pipe write end makes the child read EOF — covers the case
    // where the GUI is force-killed and never reaches terminate().
    releaseParentDeathLinkage();
}

// ── Platform: parent-death linkage ──────────────────────────────────────────

#ifdef Q_OS_UNIX
void SidecarManager::setupParentDeathLinkage(QProcess &proc,
                                             QStringList &args) {
    int fds[2];
    if (::pipe(fds) != 0) {
        qWarning("SidecarManager: pipe() failed; relying on --parent-pid only");
        return;
    }
    m_parentPipeReadFd = fds[0];
    m_parentPipeWriteFd = fds[1];

    // Only the GUI must hold the write end — set CLOEXEC so other QProcess
    // children never inherit it (else the child never sees EOF).
    ::fcntl(m_parentPipeWriteFd, F_SETFD, FD_CLOEXEC);

    const int readFd = m_parentPipeReadFd;
    proc.setChildProcessModifier([readFd]() {
        // Runs in the forked child, pre-exec. Async-signal-safe calls only.
        // Clear CLOEXEC on the read end so it survives exec into the server.
        ::fcntl(readFd, F_SETFD, 0);
    });

    args << QStringLiteral("--parent-pipe-fd") << QString::number(readFd);
}

void SidecarManager::releaseParentDeathLinkage() {
    if (m_parentPipeWriteFd >= 0) {
        ::close(m_parentPipeWriteFd);
        m_parentPipeWriteFd = -1;
    }
    if (m_parentPipeReadFd >= 0) {
        ::close(m_parentPipeReadFd);
        m_parentPipeReadFd = -1;
    }
}
#else
void SidecarManager::setupParentDeathLinkage(QProcess &, QStringList &) {}
void SidecarManager::releaseParentDeathLinkage() {}
#endif

// ── Platform: Windows Job Object (kernel-enforced reaping) ───────────────────

#ifdef Q_OS_WIN
void SidecarManager::assignToJobObjectIfWindows() {
    if (!m_jobHandle) {
        HANDLE job = ::CreateJobObjectW(nullptr, nullptr);
        if (!job) {
            qWarning("SidecarManager: CreateJobObject failed; relying on "
                     "--parent-pid only");
            return;
        }
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION info{};
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if (!::SetInformationJobObject(job, JobObjectExtendedLimitInformation,
                                       &info, sizeof(info))) {
            qWarning("SidecarManager: SetInformationJobObject failed");
            ::CloseHandle(job);
            return;
        }
        m_jobHandle = job;
    }

    const auto pid = static_cast<DWORD>(m_proc.processId());
    HANDLE hProc =
        ::OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, pid);
    if (!hProc) {
        qWarning("SidecarManager: OpenProcess(%lu) failed; --parent-pid poll "
                 "is the backstop",
                 pid);
        return;
    }
    if (!::AssignProcessToJobObject(static_cast<HANDLE>(m_jobHandle), hProc)) {
        qWarning("SidecarManager: AssignProcessToJobObject failed; "
                 "--parent-pid poll is the backstop");
    }
    ::CloseHandle(hProc);
}
#else
void SidecarManager::assignToJobObjectIfWindows() {}
#endif
