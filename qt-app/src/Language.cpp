#include "Language.h"

#include <QCoreApplication>
#include <QSettings>
#include <QTranslator>

namespace {
constexpr auto kLanguageKey = "ui/language";
constexpr auto kTranslationFile = ":/i18n/meeting-assistant_en.qm";

const QString kSystem = QStringLiteral("system");
const QString kRussian = QStringLiteral("ru");
const QString kEnglish = QStringLiteral("en");

QString normalizedChoice(const QString &choice) {
    return choice == kRussian || choice == kEnglish ? choice : kSystem;
}

QLocale firstSystemLocale(const QStringList &systemLanguages) {
    return systemLanguages.isEmpty() ? QLocale::c()
                                     : QLocale(systemLanguages.first());
}
} // namespace

Language::Language(bool readOnly, QObject *parent)
    : QObject(parent),
      m_settings(readOnly ? nullptr : std::make_unique<QSettings>()),
      m_systemLanguages(QLocale::system().uiLanguages()) {
    initialize();
}

Language::Language(const QString &settingsFile,
                   const QStringList &systemLanguages, QObject *parent)
    : QObject(parent),
      m_settings(std::make_unique<QSettings>(settingsFile, QSettings::IniFormat)),
      m_systemLanguages(systemLanguages) {
    initialize();
}

Language::~Language() = default;

void Language::initialize() {
    m_choice = m_settings ? normalizedChoice(
                                m_settings->value(QLatin1String(kLanguageKey))
                                    .toString())
                          : kSystem;
    m_active = resolve(m_choice, m_systemLanguages);
}

void Language::setChoice(const QString &choice) {
    const QString normalized = normalizedChoice(choice);
    if (normalized == m_choice)
        return;

    m_choice = normalized;
    if (m_settings) {
        m_settings->setValue(QLatin1String(kLanguageKey), m_choice);
        m_settings->sync();
    }
    emit choiceChanged();
}

bool Language::restartRequired() const {
    return resolve(m_choice, m_systemLanguages) != m_active;
}

void Language::apply() {
    QLocale::setDefault(localeFor(m_active, m_systemLanguages));

    if (m_active == kRussian)
        return; // the source strings are already Russian

    m_translator = std::make_unique<QTranslator>();
    if (m_translator->load(QLatin1String(kTranslationFile)))
        QCoreApplication::installTranslator(m_translator.get());
    else
        qWarning("Language: cannot load %s", kTranslationFile);
}

QString Language::resolve(const QString &choice,
                          const QStringList &systemLanguages) {
    if (choice == kRussian || choice == kEnglish)
        return choice;
    return firstSystemLocale(systemLanguages).language() == QLocale::Russian
               ? kRussian
               : kEnglish;
}

QLocale Language::localeFor(const QString &language,
                            const QStringList &systemLanguages) {
    const QLocale::Language wanted =
        language == kRussian ? QLocale::Russian : QLocale::English;
    const QLocale system = firstSystemLocale(systemLanguages);
    if (system.language() == wanted)
        return system;
    return wanted == QLocale::Russian
               ? QLocale(QLocale::Russian, QLocale::Russia)
               : QLocale(QLocale::English, QLocale::UnitedStates);
}
