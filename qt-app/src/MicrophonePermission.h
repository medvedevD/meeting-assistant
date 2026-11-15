#pragma once

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QObject>
#include <QPermissions>
#include <QString>

// The app's microphone permission, for the first-run microphone step.
//
// Reading `status` never brings up a system dialog, so the UI may read it at
// launch. `request()` does bring one up on macOS while the permission is
// undetermined, so it is called only from a button the user just pressed. Where
// the OS has no such permission (Windows, Linux) Qt reports it as granted.
//
// A dev build run from the build tree on macOS is not an app bundle. It has no
// usage description, so Qt can only answer "denied" for it, while the capture
// itself works under whatever launched it. There the status reads as granted
// and the microphone test is what tells the truth.
// Exposed to QML as the `microphonePermission` context property from main.cpp.
class MicrophonePermission : public QObject {
    Q_OBJECT
    // "granted" | "denied" | "undetermined"
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)

public:
    explicit MicrophonePermission(QObject *parent = nullptr) : QObject(parent) {}

    QString status() const {
        if (!m_answerable)
            return QStringLiteral("granted");
        switch (qApp->checkPermission(QMicrophonePermission{})) {
        case Qt::PermissionStatus::Granted:
            return QStringLiteral("granted");
        case Qt::PermissionStatus::Denied:
            return QStringLiteral("denied");
        case Qt::PermissionStatus::Undetermined:
            break;
        }
        return QStringLiteral("undetermined");
    }

    // Asks the OS, which asks the user if it has not yet. `statusChanged` fires
    // once the answer is in, whatever it is.
    Q_INVOKABLE void request() {
        if (!m_answerable) {
            emit statusChanged();
            return;
        }
        qApp->requestPermission(QMicrophonePermission{}, this,
                                [this](const QPermission &) { emit statusChanged(); });
    }

    // The user may have changed the answer in System Settings meanwhile, and
    // the OS does not tell the app: make QML read `status` again.
    Q_INVOKABLE void refresh() { emit statusChanged(); }

signals:
    void statusChanged();

private:
    // Whether the OS can be asked at all: always, except on macOS outside an
    // app bundle.
    static bool answerable() {
#ifdef Q_OS_MACOS
        return QFileInfo::exists(
            QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../Info.plist")));
#else
        return true;
#endif
    }

    const bool m_answerable = answerable();
};
