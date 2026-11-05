#pragma once

#include <QLocale>
#include <QObject>
#include <QString>
#include <QStringList>

#include <memory>

class QSettings;
class QTranslator;

// The interface language: what the user chose, and what this run uses.
//
// Russian is the source language of every qsTr string, so a Russian run loads no
// translation at all, and any other system language falls back to English. The
// choice lives in QSettings rather than the sidecar's settings.json because the
// GUI needs it before the sidecar is running. A run keeps one language from start
// to finish: a new choice takes effect after a restart.
class Language final : public QObject {
    Q_OBJECT
    // "system", "ru" or "en".
    Q_PROPERTY(QString choice READ choice WRITE setChoice NOTIFY choiceChanged)
    // "ru" or "en": the language of this run.
    Q_PROPERTY(QString active READ active CONSTANT)
    Q_PROPERTY(bool restartRequired READ restartRequired NOTIFY choiceChanged)

public:
    // Default QSettings + the system's language list. With `readOnly` the stored
    // choice is neither read nor changed, for the FIRST_RUN preview.
    explicit Language(bool readOnly = false, QObject *parent = nullptr);
    Language(const QString &settingsFile, const QStringList &systemLanguages,
             QObject *parent = nullptr);
    ~Language() override;

    QString choice() const { return m_choice; }
    void setChoice(const QString &choice);
    QString active() const { return m_active; }
    bool restartRequired() const;

    // Installs the translation and the default locale for `active`. Call once,
    // after the application object exists and before the QML engine loads.
    void apply();

    // `systemLanguages` is QLocale::uiLanguages(), most preferred first.
    static QString resolve(const QString &choice,
                           const QStringList &systemLanguages);
    // The locale for dates and numbers. Keeps the user's region (en_GB, ru_KZ)
    // when the system already uses `language`.
    static QLocale localeFor(const QString &language,
                             const QStringList &systemLanguages);

signals:
    void choiceChanged();

private:
    void initialize();

    std::unique_ptr<QSettings> m_settings;
    std::unique_ptr<QTranslator> m_translator;
    QStringList m_systemLanguages;
    QString m_choice;
    QString m_active;
};
