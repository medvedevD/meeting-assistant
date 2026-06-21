#include "AppPreferences.h"

#include <QSettings>

namespace {
constexpr auto kOnboardingCompletedKey = "ui/onboardingCompleted";
}

AppPreferences::AppPreferences(bool firstRunPreviewEnabled, QObject *parent)
    : QObject(parent),
      m_settings(std::make_unique<QSettings>()),
      m_firstRunPreviewEnabled(firstRunPreviewEnabled) {
    initialize();
}

AppPreferences::AppPreferences(const QString &settingsFile,
                               bool firstRunPreviewEnabled, QObject *parent)
    : QObject(parent),
      m_settings(std::make_unique<QSettings>(settingsFile, QSettings::IniFormat)),
      m_firstRunPreviewEnabled(firstRunPreviewEnabled) {
    initialize();
}

AppPreferences::~AppPreferences() = default;

bool AppPreferences::onboardingCompleted() const {
    return m_onboardingCompleted;
}

void AppPreferences::completeOnboarding() {
    if (m_firstRunPreviewEnabled || m_onboardingCompleted)
        return;

    m_onboardingCompleted = true;
    m_settings->setValue(QLatin1String(kOnboardingCompletedKey), true);
    m_settings->sync();
    emit onboardingCompletedChanged();
}

bool AppPreferences::firstRunPreviewFromEnvironment() {
    return qEnvironmentVariable("FIRST_RUN") == QLatin1String("1");
}

void AppPreferences::initialize() {
    m_onboardingCompleted =
        m_settings->value(QLatin1String(kOnboardingCompletedKey), false).toBool();
}
