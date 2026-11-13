// Offscreen Qt boot smoke (Qt-migration section-08).
//
// Drives the *real* GUI controller classes (SidecarManager + ApiClient — the
// exact code main.cpp wires) against a *real* `meeting-server` binary, under
// `QT_QPA_PLATFORM=offscreen` with a QGuiApplication so the full GUI Qt stack
// is exercised headlessly. It asserts the section-02/03 client contract:
//
//   1. spawn → handshake parse → version gate → /health gate → Ready, and an
//      authenticated GET /api/v1/meetings round-trips (handshake+health+fetch);
//   2. a simulated protocol-version mismatch drives SidecarManager to the
//      terminal `Incompatible` state and emits `versionMismatch` — precisely
//      the binding Main.qml's blocking, non-dismissable dialog is shown on
//      (`visible: sidecar.state === SidecarManager.Incompatible`), so this is
//      the blocking-dialog path asserted without a window;
//   3. the server's crash recovery stays inside the test's sandbox, so running
//      this test can never rewrite a developer's real recordings;
//   4. a failed request reaches the GUI's log stream from both sides: the
//      client's warning and the server's own error, relayed over stderr.
//
// The sidecar path comes from MA_SIDECAR (SidecarManager::locateSidecar reads
// it); the CMake `add_test` points it at the cargo-built binary. Everything the
// spawned server reads or writes stays in a private QTemporaryDir: see
// initTestCase.

#include "ApiClient.h"
#include "SidecarManager.h"

#include <QDataStream>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QScopeGuard>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>

namespace {
constexpr int kReadyTimeoutMs = 30000; // cold binary + slow CI + AV scan

// The WAV crashedRecording() builds: 100 mono float frames after a 44-byte
// header. Recovery cuts the file back to exactly this size.
constexpr qint64 kFinalizedWavBytes = 44 + 100 * 4;

// A recording whose writer died: samples on disk, both RIFF lengths still the
// placeholder 0, and a torn last frame. The server's recovery pass rewrites this
// file whenever it reaches it.
QByteArray crashedRecording() {
    QByteArray wav;
    QDataStream out(&wav, QIODevice::WriteOnly);
    out.setByteOrder(QDataStream::LittleEndian);
    out.writeRawData("RIFF", 4);
    out << quint32(0);
    out.writeRawData("WAVEfmt ", 8);
    out << quint32(16) << quint16(3) /* IEEE float */ << quint16(1) /* mono */
        << quint32(48000) << quint32(192000) /* byte rate */
        << quint16(4) /* block align */ << quint16(32) /* bits per sample */;
    out.writeRawData("data", 4);
    out << quint32(0);
    wav.append(QByteArray(100 * 4 + 3, '\0'));
    return wav;
}

[[nodiscard]] bool writeFile(const QString &path, const QByteArray &bytes) {
    if (!QFileInfo(path).dir().mkpath(QStringLiteral(".")))
        return false;
    QFile file(path);
    return file.open(QIODevice::WriteOnly) &&
           file.write(bytes) == bytes.size();
}

QByteArray readFile(const QString &path) {
    QFile file(path);
    return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
}

// What the process logs while collectMessages() is installed. The GUI's file
// logger (Logging.cpp) tees this same stream into meeting-assistant.log.
QStringList g_messages;
QtMessageHandler g_chainedHandler = nullptr;

void collectMessages(QtMsgType type, const QMessageLogContext &ctx,
                     const QString &msg) {
    g_messages.append(msg);
    if (g_chainedHandler)
        g_chainedHandler(type, ctx, msg);
}

bool logged(const QStringList &needles) {
    for (const QString &msg : g_messages) {
        bool all = true;
        for (const QString &needle : needles)
            all = all && msg.contains(needle);
        if (all)
            return true;
    }
    return false;
}
} // namespace

class SmokeTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void handshakeHealthAndMeetingsFetch();
    void versionMismatchTriggersBlockingDialogPath();
    void crashRecoveryStaysInsideTheSandbox();
    void failedStopIsLogged();

private:
    QTemporaryDir m_sandbox;
};

void SmokeTest::initTestCase() {
    const QByteArray sidecar = qgetenv("MA_SIDECAR");
    QVERIFY2(!sidecar.isEmpty(),
             "MA_SIDECAR must point at the cargo-built meeting-server "
             "(set by the CMake add_test / CI step)");
    QVERIFY2(QFileInfo::exists(QString::fromLocal8Bit(sidecar)),
             qPrintable(QStringLiteral("meeting-server not found at %1")
                            .arg(QString::fromLocal8Bit(sidecar))));

    QVERIFY(m_sandbox.isValid());
    const QDir sandbox(m_sandbox.path());
    const auto inSandbox = [&sandbox](const QString &name) {
        return sandbox.filePath(name).toLocal8Bit();
    };

    // On every OS these hold the DB, the singleton lock, settings.json and the
    // secrets file, so a real install or running app is never touched.
    qputenv("XDG_DATA_HOME", inSandbox(QStringLiteral("data")));
    qputenv("XDG_CACHE_HOME", inSandbox(QStringLiteral("cache")));
    qputenv("XDG_CONFIG_HOME", inSandbox(QStringLiteral("config")));
    qputenv("HOME", inSandbox(QStringLiteral("home")));
    // Otherwise the server round-trips a probe through the real OS keyring.
    qputenv("MEETING_ASSISTANT_KEYRING_DISABLE", QByteArrayLiteral("1"));
    qunsetenv("ANTHROPIC_API_KEY");
    qunsetenv("MA_FORCE_CLIENT_PROTOCOL");

    // None of the above covers the recordings dir. It defaults to the OS
    // Documents folder, which follows HOME on Linux and macOS and no env var at
    // all on Windows, and the server's recovery pass rewrites damaged WAVs in
    // it. This setting is what moves it on every OS.
    const QJsonObject settings{
        {QStringLiteral("paths"),
         QJsonObject{{QStringLiteral("meetings_dir"),
                      sandbox.filePath(QStringLiteral("recordings"))}}},
        {QStringLiteral("recording"),
         QJsonObject{{QStringLiteral("source"), QStringLiteral("mic")},
                     {QStringLiteral("echo_cancel"), false}}},
    };
    QVERIFY(writeFile(sandbox.filePath(QStringLiteral(
                          "config/meeting-assistant/settings.json")),
                      QJsonDocument(settings).toJson()));
}

// 1 — full happy path: spawn, handshake, version gate, /health, then an
//     authenticated meetings fetch round-trips.
void SmokeTest::handshakeHealthAndMeetingsFetch() {
    SidecarManager sidecar;
    ApiClient api;
    // Exactly the wiring main.cpp does.
    QObject::connect(&sidecar, &SidecarManager::ready, &api,
                     &ApiClient::configure);

    sidecar.start();
    QTRY_VERIFY_WITH_TIMEOUT(sidecar.state() == SidecarManager::Ready,
                             kReadyTimeoutMs);
    QVERIFY(api.isConfigured());

    QSignalSpy ok(&api, &ApiClient::requestSucceeded);
    QSignalSpy failed(&api, &ApiClient::requestFailed);
    const int reqId = api.get(QStringLiteral("/api/v1/meetings"));

    QTRY_VERIFY_WITH_TIMEOUT(ok.count() + failed.count() == 1, 15000);
    QCOMPARE(failed.count(), 0); // bearer auth + handler must succeed
    QCOMPARE(ok.count(), 1);

    const QList<QVariant> args = ok.takeFirst();
    QCOMPARE(args.at(0).toInt(), reqId);
    QCOMPARE(args.at(1).toString(), QStringLiteral("/api/v1/meetings"));
    // A fresh DB → an empty list, but it must be a JSON array (round-trip).
    QVERIFY2(args.at(2).typeId() == QMetaType::QVariantList,
             "GET /api/v1/meetings must return a JSON array");

    // Reap the child (synchronous: terminate → waitForFinished → kill). By
    // design SidecarManager keeps its state on an intentional shutdown — the
    // only assertion that matters here is that the server is gone so the next
    // test can re-acquire the singleton on the shared data dir.
    sidecar.shutdown();
}

// 2 — Q9 version gate: a client protocol outside the server's [min,max] must
//     end in the terminal Incompatible state + versionMismatch signal — the
//     exact trigger of Main.qml's blocking dialog.
void SmokeTest::versionMismatchTriggersBlockingDialogPath() {
    // Read in the SidecarManager ctor; force an out-of-range client protocol.
    qputenv("MA_FORCE_CLIENT_PROTOCOL", QByteArrayLiteral("999"));

    SidecarManager sidecar;
    QSignalSpy mismatch(&sidecar, &SidecarManager::versionMismatch);
    QSignalSpy readySpy(&sidecar, &SidecarManager::ready);

    sidecar.start();
    QTRY_VERIFY_WITH_TIMEOUT(
        sidecar.state() == SidecarManager::Incompatible, kReadyTimeoutMs);

    QCOMPARE(mismatch.count(), 1);
    const QList<QVariant> a = mismatch.takeFirst();
    QCOMPARE(a.at(0).toUInt(), 999u); // client protocol we forced
    // server min <= server max, and 999 is outside it.
    const uint sMin = a.at(1).toUInt();
    const uint sMax = a.at(2).toUInt();
    QVERIFY(sMin <= sMax);
    QVERIFY(999u < sMin || 999u > sMax);

    // It must NOT have proceeded to a usable state.
    QCOMPARE(readySpy.count(), 0);
    QVERIFY(sidecar.state() != SidecarManager::Ready);

    qunsetenv("MA_FORCE_CLIENT_PROTOCOL");
}

// 3 — the recovery pass reaches the sandbox's recordings dir and nothing else.
//     An exact list matters on a developer machine: a scan of the real folder
//     would add a row for each real recording.
void SmokeTest::crashRecoveryStaysInsideTheSandbox() {
    const QString meeting =
        QStringLiteral("2026-05-10_14-30_a1b2c3d4/recording.wav");
    const QDir sandbox(m_sandbox.path());
    const QString configured =
        sandbox.filePath(QStringLiteral("recordings/") + meeting);
    // Where the default resolves under the sandbox's HOME on Linux and macOS. On
    // Windows nothing can reach this copy, so `configured` carries the test.
    const QString atDefault = sandbox.filePath(
        QStringLiteral("home/Documents/meeting-assistant/") + meeting);
    QVERIFY(writeFile(configured, crashedRecording()));
    QVERIFY(writeFile(atDefault, crashedRecording()));

    SidecarManager sidecar;
    ApiClient api;
    QObject::connect(&sidecar, &SidecarManager::ready, &api,
                     &ApiClient::configure);

    // The handshake is printed after the recovery pass, so Ready means it ran.
    sidecar.start();
    QTRY_VERIFY_WITH_TIMEOUT(sidecar.state() == SidecarManager::Ready,
                             kReadyTimeoutMs);

    QSignalSpy ok(&api, &ApiClient::requestSucceeded);
    QSignalSpy failed(&api, &ApiClient::requestFailed);
    api.get(QStringLiteral("/api/v1/meetings"));
    QTRY_VERIFY_WITH_TIMEOUT(ok.count() + failed.count() == 1, 15000);
    QCOMPARE(failed.count(), 0);

    const QVariantList meetings = ok.takeFirst().at(2).toList();
    QCOMPARE(meetings.size(), 1);
    const QVariantMap listed = meetings.first().toMap();
    QCOMPARE(QDir::fromNativeSeparators(
                 listed.value(QStringLiteral("audio_path")).toString()),
             configured);

    QCOMPARE(QFileInfo(configured).size(), kFinalizedWavBytes);
    QCOMPARE(readFile(atDefault), crashedRecording());
}

// 4 — a stop the server refuses (409, as when the capture could not be
//     closed) is in the log from both sides, so a tester's log explains it.
void SmokeTest::failedStopIsLogged() {
    SidecarManager sidecar;
    ApiClient api;
    QObject::connect(&sidecar, &SidecarManager::ready, &api,
                     &ApiClient::configure);
    sidecar.start();
    QTRY_VERIFY_WITH_TIMEOUT(sidecar.state() == SidecarManager::Ready,
                             kReadyTimeoutMs);

    g_messages.clear();
    g_chainedHandler = qInstallMessageHandler(collectMessages);
    const auto restore =
        qScopeGuard([] { qInstallMessageHandler(g_chainedHandler); });

    QSignalSpy failed(&api, &ApiClient::requestFailed);
    const QString path =
        QStringLiteral("/api/v1/recordings/no-such-session/stop");
    api.post(path, {});
    QTRY_COMPARE_WITH_TIMEOUT(failed.count(), 1, 15000);
    QCOMPARE(failed.first().at(2).toInt(), 409);

    QTRY_VERIFY_WITH_TIMEOUT(
        logged({QStringLiteral("ApiClient: POST ") + path,
                QStringLiteral("failed (HTTP 409)")}),
        5000);
    QTRY_VERIFY_WITH_TIMEOUT(
        logged({QStringLiteral("[meeting-server]"), QStringLiteral("ERROR"),
                QStringLiteral("stop recording failed"),
                QStringLiteral("meeting_id=no-such-session")}),
        5000);

    sidecar.shutdown();
}

// QGuiApplication (not guiless): exercise the real GUI Qt stack under the
// offscreen platform, the way the shipped app runs.
int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    SmokeTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_smoke.moc"
