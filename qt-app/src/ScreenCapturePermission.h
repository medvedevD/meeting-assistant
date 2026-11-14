#pragma once

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QObject>
#include <QProcess>
#include <QSettings>
#include <QString>

// Resets this app's Screen Recording permission, so that macOS asks again.
//
// Release builds are ad-hoc signed, so after an update macOS keeps the grant for
// the previous binary: the app's switch in System Settings shows on, and the
// recording still fails. Removing the app there and adding it back did not help
// in the alpha.15 smoke run; `tccutil reset ScreenCapture <bundle id>` did. An
// app may reset its own entries without admin rights. Exposed to QML as the
// `screenCapturePermission` context property from main.cpp.
class ScreenCapturePermission : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool canReset READ canReset CONSTANT)

public:
    explicit ScreenCapturePermission(QObject *parent = nullptr)
        : QObject(parent), m_bundleId(appBundleId()) {}

    bool canReset() const { return !m_bundleId.isEmpty(); }

    // True once macOS forgot the grant. The next capture then brings up its
    // prompt, and granting there asks to quit and reopen the app.
    Q_INVOKABLE bool reset() const {
        if (!canReset())
            return false;
        QProcess tccutil;
        tccutil.start(QStringLiteral("/usr/bin/tccutil"),
                      {QStringLiteral("reset"), QStringLiteral("ScreenCapture"), m_bundleId});
        const bool ok = tccutil.waitForFinished(5000) && tccutil.exitStatus() == QProcess::NormalExit
                        && tccutil.exitCode() == 0;
        if (!ok)
            qWarning("ScreenCapturePermission: tccutil reset failed: %s",
                     qPrintable(QString::fromUtf8(tccutil.readAllStandardError()).trimmed()));
        return ok;
    }

private:
    // The bundle identifier from the app's Info.plist. Empty outside a macOS app
    // bundle (a dev build run from the build tree), which has no grant to reset.
    static QString appBundleId() {
#ifdef Q_OS_MACOS
        const QString plist =
            QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../Info.plist"));
        if (!QFileInfo::exists(plist))
            return {};
        return QSettings(plist, QSettings::NativeFormat)
            .value(QStringLiteral("CFBundleIdentifier"))
            .toString();
#else
        return {};
#endif
    }

    const QString m_bundleId;
};
