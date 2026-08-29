#pragma once

#include <QObject>
#include <QString>

#include <memory>

class QSettings;

class AppPreferences final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool onboardingCompleted READ onboardingCompleted NOTIFY
                   onboardingCompletedChanged)
    Q_PROPERTY(bool firstRunPreviewEnabled READ firstRunPreviewEnabled CONSTANT)

public:
    explicit AppPreferences(bool firstRunPreviewEnabled = false,
                            QObject *parent = nullptr);
    AppPreferences(const QString &settingsFile, bool firstRunPreviewEnabled,
                   QObject *parent = nullptr);
    ~AppPreferences() override;

    bool onboardingCompleted() const;
    bool firstRunPreviewEnabled() const { return m_firstRunPreviewEnabled; }

    Q_INVOKABLE void completeOnboarding();

    static bool firstRunPreviewFromEnvironment();

signals:
    void onboardingCompletedChanged();

private:
    void initialize();

    std::unique_ptr<QSettings> m_settings;
    bool m_onboardingCompleted = false;
    bool m_firstRunPreviewEnabled = false;
};
