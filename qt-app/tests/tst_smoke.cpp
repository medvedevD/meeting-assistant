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
//      the blocking-dialog path asserted without a window.
//
// The sidecar path comes from MA_SIDECAR (SidecarManager::locateSidecar reads
// it); the CMake `add_test` points it at the cargo-built binary. A private
// QTemporaryDir is exported as XDG_DATA_HOME/XDG_CACHE_HOME so the spawned
// server's DB, recordings dir and singleton lock never touch a real install.

#include "ApiClient.h"
#include "SidecarManager.h"

#include <QFileInfo>
#include <QGuiApplication>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>

namespace {
constexpr int kReadyTimeoutMs = 30000; // cold binary + slow CI + AV scan
}

class SmokeTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void handshakeHealthAndMeetingsFetch();
    void versionMismatchTriggersBlockingDialogPath();

private:
    QTemporaryDir m_dataDir;
};

void SmokeTest::initTestCase() {
    const QByteArray sidecar = qgetenv("MA_SIDECAR");
    QVERIFY2(!sidecar.isEmpty(),
             "MA_SIDECAR must point at the cargo-built meeting-server "
             "(set by the CMake add_test / CI step)");
    QVERIFY2(QFileInfo::exists(QString::fromLocal8Bit(sidecar)),
             qPrintable(QStringLiteral("meeting-server not found at %1")
                            .arg(QString::fromLocal8Bit(sidecar))));

    QVERIFY(m_dataDir.isValid());
    // Isolate the spawned server's state from any real install / running app
    // (the singleton lock lives under XDG_DATA_HOME).
    qputenv("XDG_DATA_HOME", m_dataDir.path().toLocal8Bit());
    qputenv("XDG_CACHE_HOME", m_dataDir.path().toLocal8Bit());
    qunsetenv("ANTHROPIC_API_KEY");
    qunsetenv("MA_FORCE_CLIENT_PROTOCOL");
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

// QGuiApplication (not guiless): exercise the real GUI Qt stack under the
// offscreen platform, the way the shipped app runs.
int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    SmokeTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_smoke.moc"
