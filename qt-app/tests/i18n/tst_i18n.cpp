// Interface language: how it is resolved and stored, that the embedded English
// translation reaches QML (including the errors.js library), and that the QML
// sources hide no Russian text or old product name from the translation.
//
// The other QML tests load no translator on purpose: their Russian assertions
// check the source strings.
#include "EmbeddedQmldirGuard.h"
#include "JobPoller.h"
#include "Language.h"

#include <QDirIterator>
#include <QFile>
#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QRegularExpression>
#include <QSet>
#include <QSettings>
#include <QTemporaryDir>
#include <QtTest>

#include <memory>

namespace {

bool hasCyrillic(const QString &text) {
    static const QRegularExpression cyrillic(QStringLiteral("[\\x{0400}-\\x{04FF}]"));
    return cyrillic.match(text).hasMatch();
}

// A string literal in a QML or JS file, and whether it is an argument of qsTr.
struct StringLiteral {
    QString text;
    int line = 1;
    bool translated = false;
};

// True when a `/` here starts a regular expression rather than a division, judged
// by what precedes it: an operand cannot end in one of these characters.
bool startsRegExp(QChar previous, const QString &lastWord) {
    return previous.isNull() || lastWord == u"return"
           || QStringView(u"(,=:[!&|?{};+-*%<>~^\n").contains(previous);
}

// Skips a regular expression literal, whose brackets and quotes are not code.
// `i` is at the opening `/` and ends at the closing one.
void skipRegExp(const QString &source, qsizetype &i) {
    bool inClass = false;
    for (++i; i < source.size() && source[i] != u'\n'; ++i) {
        if (source[i] == u'\\')
            ++i;
        else if (source[i] == u'[')
            inClass = true;
        else if (source[i] == u']')
            inClass = false;
        else if (source[i] == u'/' && !inClass)
            return;
    }
}

// Comments and regular expressions are skipped and literals collected, so a word
// in a comment does not count and a multi-line qsTr("a" + "b") does.
QList<StringLiteral> stringLiterals(const QString &source) {
    QList<StringLiteral> literals;
    QList<int> qsTrDepths;
    QString word;
    QString lastWord;
    QChar previous;
    int depth = 0;
    int line = 1;

    for (qsizetype i = 0; i < source.size(); ++i) {
        const QChar c = source[i];
        if (c.isLetterOrNumber() || c == u'_') {
            word += c;
            previous = c;
            continue;
        }
        if (!word.isEmpty()) {
            lastWord = word;
            word.clear();
        }

        const QChar next = i + 1 < source.size() ? source[i + 1] : QChar();
        if (c == u'\n') {
            ++line;
            previous = c;
        } else if (c == u'/' && next == u'/') {
            while (i + 1 < source.size() && source[i + 1] != u'\n')
                ++i;
        } else if (c == u'/' && next == u'*') {
            for (i += 2; i + 1 < source.size(); ++i) {
                if (source[i] == u'*' && source[i + 1] == u'/')
                    break;
                if (source[i] == u'\n')
                    ++line;
            }
            ++i;
        } else if (c == u'/' && startsRegExp(previous, lastWord)) {
            skipRegExp(source, i);
            previous = u'/';
            lastWord.clear();
        } else if (c == u'"' || c == u'\'' || c == u'`') {
            StringLiteral literal;
            literal.line = line;
            literal.translated = !qsTrDepths.isEmpty();
            for (++i; i < source.size() && source[i] != c; ++i) {
                if (source[i] == u'\\')
                    ++i;
                else if (source[i] == u'\n')
                    ++line;
                else
                    literal.text += source[i];
            }
            literals.append(literal);
            previous = c;
            lastWord.clear();
        } else if (!c.isSpace()) {
            if (c == u'(') {
                ++depth;
                if (lastWord == u"qsTr")
                    qsTrDepths.append(depth);
            } else if (c == u')') {
                if (!qsTrDepths.isEmpty() && qsTrDepths.last() == depth)
                    qsTrDepths.removeLast();
                --depth;
            }
            previous = c;
            lastWord.clear();
        }
    }
    return literals;
}

QStringList appQmlFiles() {
    QStringList files;
    QDirIterator it(QStringLiteral(MA_APP_QML_SOURCE_DIR),
                    {QStringLiteral("*.qml"), QStringLiteral("*.js")},
                    QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext())
        files.append(it.next());
    // Reached only with MEETY_THEME_GALLERY=1, a developer's token sign-off view.
    files.removeIf([](const QString &file) {
        return file.endsWith(QStringLiteral("/ThemeGallery.qml"));
    });
    return files;
}

QString readAll(const QString &path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return {};
    return QString::fromUtf8(file.readAll());
}

} // namespace

class I18nTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void cleanup();

    void resolvesTheLanguage_data();
    void resolvesTheLanguage();
    void choiceSurvivesARestartAndAsksForOne();
    void unknownStoredChoiceMeansSystem();
    void readOnlyLanguageKeepsTheChoiceInMemory();
    void localeKeepsTheRegionOfAMatchingSystem_data();
    void localeKeepsTheRegionOfAMatchingSystem();

    void russianRunLoadsNoTranslation();
    void errorsLibraryIsTranslated();
    void errorsLibraryStaysRussianWithoutATranslation();
    void pipelineLabelsComeFromTheStage_data();
    void pipelineLabelsComeFromTheStage();
    void datesFollowTheLanguage_data();
    void datesFollowTheLanguage();
    void defaultTemplateLabelNamesTheEnglishTemplate();
    void selectorStoresTheChoice();

    void literalScannerSeparatesCodeFromText();
    void qmlHasNoUntranslatedRussian();
    void qmlShowsNoOldProductName();
    void qmlDatesDoNotBypassTheLocale();

private:
    std::unique_ptr<Language> appliedLanguage(const QStringList &systemLanguages);
    std::unique_ptr<QObject> errorsHost(QQmlEngine &engine);

    QTemporaryDir m_dir;
    QLocale m_initialLocale;
    int m_settingsFiles = 0;
};

void I18nTest::initTestCase() {
    QVERIFY(m_dir.isValid());
    m_initialLocale = QLocale();
    verifyEmbeddedQmldir(QStringLiteral(MA_I18N_QML_IMPORT_DIR));
    qmlRegisterType<JobPoller>("MeetingAssistant", 1, 0, "JobPoller");
}

void I18nTest::cleanup() {
    QLocale::setDefault(m_initialLocale);
}

// A fresh profile on a system with these languages, applied. The translation it
// installs is removed again when the returned object goes away.
std::unique_ptr<Language> I18nTest::appliedLanguage(const QStringList &systemLanguages) {
    const QString settings =
        m_dir.filePath(QStringLiteral("applied-%1.ini").arg(++m_settingsFiles));
    auto language = std::make_unique<Language>(settings, systemLanguages);
    language->apply();
    return language;
}

// A QML object that exposes errors.js, the way screens import it.
std::unique_ptr<QObject> I18nTest::errorsHost(QQmlEngine &engine) {
    engine.addImportPath(QStringLiteral(MA_I18N_QML_IMPORT_DIR));
    QQmlComponent component(&engine);
    component.setData(R"qml(
        import QtQuick
        import "i18n/errors.js" as Errors
        QtObject {
            function describe(errorClass, message) {
                return Errors.describe(errorClass, message)
            }
        })qml",
                      QUrl::fromLocalFile(QStringLiteral(MA_I18N_QML_DIR "/host.qml")));
    if (!component.isReady())
        qWarning().noquote() << component.errorString();
    return std::unique_ptr<QObject>(component.create());
}

void I18nTest::resolvesTheLanguage_data() {
    QTest::addColumn<QString>("choice");
    QTest::addColumn<QStringList>("system");
    QTest::addColumn<QString>("expected");

    const QString sys = QStringLiteral("system");
    const QString ru = QStringLiteral("ru");
    const QString en = QStringLiteral("en");
    QTest::newRow("ru_RU") << sys << QStringList{"ru-RU"} << ru;
    QTest::newRow("en_US") << sys << QStringList{"en-US"} << en;
    QTest::newRow("de_DE") << sys << QStringList{"de-DE"} << en;
    QTest::newRow("underscore tag") << sys << QStringList{"ru_RU"} << ru;
    QTest::newRow("bare language") << sys << QStringList{"ru"} << ru;
    QTest::newRow("script in the tag") << sys << QStringList{"ru-Cyrl-RU"} << ru;
    QTest::newRow("first language decides") << sys << QStringList{"en-US", "ru-RU"} << en;
    QTest::newRow("russian first") << sys << QStringList{"ru-RU", "en-US"} << ru;
    QTest::newRow("no system language") << sys << QStringList{} << en;
    QTest::newRow("explicit english on russian") << en << QStringList{"ru-RU"} << en;
    QTest::newRow("explicit russian on english") << ru << QStringList{"en-US"} << ru;
    QTest::newRow("explicit russian on german") << ru << QStringList{"de-DE"} << ru;
}

void I18nTest::resolvesTheLanguage() {
    QFETCH(QString, choice);
    QFETCH(QStringList, system);
    QFETCH(QString, expected);
    QCOMPARE(Language::resolve(choice, system), expected);
}

void I18nTest::choiceSurvivesARestartAndAsksForOne() {
    const QString settings = m_dir.filePath(QStringLiteral("restart.ini"));
    const QStringList russianSystem{QStringLiteral("ru-RU")};

    Language first(settings, russianSystem);
    QCOMPARE(first.choice(), QStringLiteral("system"));
    QCOMPARE(first.active(), QStringLiteral("ru"));
    QVERIFY(!first.restartRequired());

    QSignalSpy changed(&first, &Language::choiceChanged);
    first.setChoice(QStringLiteral("en"));
    QCOMPARE(changed.count(), 1);
    QCOMPARE(first.choice(), QStringLiteral("en"));
    QCOMPARE(first.active(), QStringLiteral("ru")); // one language per run
    QVERIFY(first.restartRequired());

    first.setChoice(QStringLiteral("en"));
    QCOMPARE(changed.count(), 1);

    Language restarted(settings, russianSystem);
    QCOMPARE(restarted.choice(), QStringLiteral("en"));
    QCOMPARE(restarted.active(), QStringLiteral("en"));
    QVERIFY(!restarted.restartRequired());

    // Going back to "system" on a Russian system needs a restart again, while
    // "ru" and "system" resolve alike and do not.
    restarted.setChoice(QStringLiteral("system"));
    QVERIFY(restarted.restartRequired());
    Language back(settings, russianSystem);
    QCOMPARE(back.active(), QStringLiteral("ru"));
    back.setChoice(QStringLiteral("ru"));
    QVERIFY(!back.restartRequired());
}

void I18nTest::unknownStoredChoiceMeansSystem() {
    const QString settings = m_dir.filePath(QStringLiteral("unknown.ini"));
    {
        QSettings raw(settings, QSettings::IniFormat);
        raw.setValue(QStringLiteral("ui/language"), QStringLiteral("de"));
    }
    Language language(settings, {QStringLiteral("ru-RU")});
    QCOMPARE(language.choice(), QStringLiteral("system"));
    QCOMPARE(language.active(), QStringLiteral("ru"));
}

// The FIRST_RUN preview must not touch the profile it was started from.
void I18nTest::readOnlyLanguageKeepsTheChoiceInMemory() {
    Language preview(true);
    QCOMPARE(preview.choice(), QStringLiteral("system"));
    preview.setChoice(QStringLiteral("en"));
    QCOMPARE(preview.choice(), QStringLiteral("en"));

    Language next(true);
    QCOMPARE(next.choice(), QStringLiteral("system"));
}

void I18nTest::localeKeepsTheRegionOfAMatchingSystem_data() {
    QTest::addColumn<QString>("language");
    QTest::addColumn<QStringList>("system");
    QTest::addColumn<QString>("expected");

    QTest::newRow("en on en_GB") << "en" << QStringList{"en-GB"} << "en_GB";
    QTest::newRow("en on de_DE") << "en" << QStringList{"de-DE"} << "en_US";
    QTest::newRow("ru on ru_KZ") << "ru" << QStringList{"ru-KZ"} << "ru_KZ";
    QTest::newRow("ru on en_US") << "ru" << QStringList{"en-US"} << "ru_RU";
}

void I18nTest::localeKeepsTheRegionOfAMatchingSystem() {
    QFETCH(QString, language);
    QFETCH(QStringList, system);
    QFETCH(QString, expected);
    QCOMPARE(Language::localeFor(language, system).name(), expected);
}

void I18nTest::russianRunLoadsNoTranslation() {
    const auto language = appliedLanguage({QStringLiteral("ru-RU")});
    QCOMPARE(QCoreApplication::translate("errors", "Сетевая ошибка"),
             QStringLiteral("Сетевая ошибка"));
    QCOMPARE(QLocale().language(), QLocale::Russian);
}

// Decision on the plan's risk: qsTr has to work in a `.pragma library` file.
void I18nTest::errorsLibraryIsTranslated() {
    const auto language = appliedLanguage({QStringLiteral("en-US")});
    QCOMPARE(QCoreApplication::translate("errors", "Сетевая ошибка"),
             QStringLiteral("Network error"));

    QQmlEngine engine;
    const auto host = errorsHost(engine);
    QVERIFY(host != nullptr);

    const QStringList classes{QStringLiteral("api_auth"),
                              QStringLiteral("llm_model_unavailable"),
                              QStringLiteral("llm_rejected"),
                              QStringLiteral("api_quota"),
                              QStringLiteral("network_timeout"),
                              QStringLiteral("audio_corrupt"),
                              QStringLiteral("model_missing"),
                              QStringLiteral("model_not_selected"),
                              QStringLiteral("worker_killed"),
                              QStringLiteral("cancelled"),
                              QStringLiteral("unknown"),
                              QStringLiteral("something_new")};
    for (const QString &errorClass : classes) {
        QVariant result;
        QVERIFY(QMetaObject::invokeMethod(host.get(), "describe",
                                          Q_RETURN_ARG(QVariant, result),
                                          Q_ARG(QVariant, errorClass),
                                          Q_ARG(QVariant, QString())));
        const QVariantMap described = result.toMap();
        const QString title = described.value(QStringLiteral("title")).toString();
        const QString hint = described.value(QStringLiteral("hint")).toString();
        QVERIFY2(!title.isEmpty() && !hasCyrillic(title), qPrintable(errorClass + ": " + title));
        QVERIFY2(!hint.isEmpty() && !hasCyrillic(hint), qPrintable(errorClass + ": " + hint));
    }

    QVariant authResult;
    QVERIFY(QMetaObject::invokeMethod(host.get(), "describe",
                                      Q_RETURN_ARG(QVariant, authResult),
                                      Q_ARG(QVariant, QStringLiteral("api_auth")),
                                      Q_ARG(QVariant, QString())));
    QCOMPARE(authResult.toMap().value(QStringLiteral("title")).toString(),
             QStringLiteral("LLM is not configured"));
}

void I18nTest::errorsLibraryStaysRussianWithoutATranslation() {
    QQmlEngine engine;
    const auto host = errorsHost(engine);
    QVERIFY(host != nullptr);

    QVariant result;
    QVERIFY(QMetaObject::invokeMethod(host.get(), "describe",
                                      Q_RETURN_ARG(QVariant, result),
                                      Q_ARG(QVariant, QStringLiteral("api_auth")),
                                      Q_ARG(QVariant, QString())));
    QCOMPARE(result.toMap().value(QStringLiteral("title")).toString(),
             QStringLiteral("LLM не настроена"));
}

void I18nTest::pipelineLabelsComeFromTheStage_data() {
    QTest::addColumn<QStringList>("system");
    QTest::addColumn<QString>("stage");
    QTest::addColumn<QString>("expected");

    // The backend `sub` in the test is Russian, as older builds sent it.
    const QStringList en{QStringLiteral("en-US")};
    const QStringList ru{QStringLiteral("ru-RU")};
    QTest::newRow("en queued") << en << "queued" << "Queued…";
    QTest::newRow("en loading") << en << "loading_model" << "Loading model…";
    QTest::newRow("en decoding") << en << "decoding_audio" << "Preparing audio…";
    QTest::newRow("en transcribing") << en << "transcribing" << "Whisper is recognizing speech…";
    QTest::newRow("en saving") << en << "writing_transcript" << "Saving the transcript…";
    QTest::newRow("en minutes") << en << "generating_protocol" << "The LLM is writing the minutes…";
    QTest::newRow("ru transcribing") << ru << "transcribing" << "Whisper распознаёт речь…";
}

void I18nTest::pipelineLabelsComeFromTheStage() {
    QFETCH(QStringList, system);
    QFETCH(QString, stage);
    QFETCH(QString, expected);

    const auto language = appliedLanguage(system);
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_I18N_QML_IMPORT_DIR));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(MA_I18N_QML_DIR "/components/PipelineProgress.qml")));
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));
    std::unique_ptr<QObject> root(component.create());
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    const QVariantMap job{
        {QStringLiteral("status"), QStringLiteral("running")},
        {QStringLiteral("progress"),
         QVariantMap{{QStringLiteral("stage"), stage},
                     {QStringLiteral("sub"), QStringLiteral("Распознавание речи")},
                     {QStringLiteral("percent"), 40}}}};
    root->setProperty("sourceJob", job);

    QVariant subline;
    QVERIFY(QMetaObject::invokeMethod(root.get(), "statusSubline",
                                      Q_RETURN_ARG(QVariant, subline)));
    QCOMPARE(subline.toString(), expected);

    if (language->active() == QStringLiteral("en")) {
        const auto children = root->findChildren<QObject *>();
        for (QObject *child : children) {
            const QString text = child->property("text").toString();
            QVERIFY2(!hasCyrillic(text), qPrintable(text));
        }
    }
}

void I18nTest::datesFollowTheLanguage_data() {
    QTest::addColumn<QStringList>("system");
    QTest::addColumn<QString>("format");
    QTest::addColumn<QString>("expected");

    const QStringList en{QStringLiteral("en-US")};
    const QStringList ru{QStringLiteral("ru-RU")};
    QTest::newRow("en full") << en << "d MMMM yyyy" << "5 November 2026";
    QTest::newRow("en short") << en << "d MMM" << "5 Nov";
    QTest::newRow("en weekday") << en << "ddd" << "Thu";
    QTest::newRow("ru full") << ru << "d MMMM yyyy" << "5 ноября 2026";
    QTest::newRow("ru time") << ru << "d MMMM, HH:mm" << "5 ноября, 14:30";
}

void I18nTest::datesFollowTheLanguage() {
    QFETCH(QStringList, system);
    QFETCH(QString, format);
    QFETCH(QString, expected);

    const auto language = appliedLanguage(system);
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"qml(
        import QtQml
        QtObject {
            function format(pattern) {
                return new Date(2026, 10, 5, 14, 30).toLocaleString(Qt.locale(), pattern)
            }
        })qml",
                      QUrl());
    std::unique_ptr<QObject> host(component.create());
    QVERIFY2(host != nullptr, qPrintable(component.errorString()));

    QVariant result;
    QVERIFY(QMetaObject::invokeMethod(host.get(), "format",
                                      Q_RETURN_ARG(QVariant, result),
                                      Q_ARG(QVariant, format)));
    QCOMPARE(result.toString(), expected);
}

void I18nTest::defaultTemplateLabelNamesTheEnglishTemplate() {
    const auto language = appliedLanguage({QStringLiteral("en-US")});
    const QString label =
        QCoreApplication::translate("GenerateProtocolScreen", "Простой протокол");
    QVERIFY(!hasCyrillic(label));
    const QString file = QStringLiteral(MA_PROMPTS_DIR "/en/") + label + QStringLiteral(".md");
    QVERIFY2(QFile::exists(file), qPrintable(file));
}

void I18nTest::selectorStoresTheChoice() {
    const QString settings = m_dir.filePath(QStringLiteral("selector.ini"));
    const QStringList russianSystem{QStringLiteral("ru-RU")};
    Language language(settings, russianSystem);

    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_I18N_QML_IMPORT_DIR));
    engine.rootContext()->setContextProperty(QStringLiteral("appLanguage"), &language);
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(MA_I18N_QML_DIR "/screens/settings/LanguagePanel.qml")));
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));
    std::unique_ptr<QObject> root(component.create());
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    auto *box = root->findChild<QObject *>(QStringLiteral("languageBox"));
    QVERIFY(box != nullptr);
    QCOMPARE(box->property("currentValue").toString(), QStringLiteral("system"));

    box->setProperty("currentIndex", 2);
    QVERIFY(QMetaObject::invokeMethod(box, "activated", Q_ARG(int, 2)));
    QCOMPARE(language.choice(), QStringLiteral("en"));
    QVERIFY(language.restartRequired());

    Language restarted(settings, russianSystem);
    QCOMPARE(restarted.active(), QStringLiteral("en"));
}

// The scans below are only as good as this, and a wrong parse fails silently.
void I18nTest::literalScannerSeparatesCodeFromText() {
    const auto literals = stringLiterals(QStringLiteral(R"js(
        // "в комментарии"
        /* "в блоке" */
        Text { text: "plain"; other: qsTr("обёрнуто" + " дважды").arg("аргумент") }
        var a = /`[^`"]+`/.exec(t), b = x / 2, c = "после"
        return /"/.test(s) ? 'одинарные' : `обратные`
    )js"));

    QStringList plain;
    QStringList translated;
    for (const StringLiteral &literal : literals)
        (literal.translated ? translated : plain) << literal.text;

    QCOMPARE(translated, (QStringList{"обёрнуто", " дважды"}));
    QCOMPARE(plain, (QStringList{"plain", "аргумент", "после", "одинарные", "обратные"}));
    QCOMPARE(literals.first().line, 4);
}

// lupdate cannot see a string outside qsTr, so the translation check would pass
// while the English interface shows Russian.
void I18nTest::qmlHasNoUntranslatedRussian() {
    // Never shown: the markers ProtocolDocument compares model output against.
    // "Русский" is a language's own name in the selector, kept in every language.
    const QSet<QString> allowed{
        QStringLiteral("нет"),           QStringLiteral("не применимо"),
        QStringLiteral("отсутствует"),   QStringLiteral("отсутствуют"),
        QStringLiteral("не обсуждалось"), QStringLiteral("не было"),
        QStringLiteral("не выявлено"),   QStringLiteral("нет данных"),
        QStringLiteral("не указано"),    QStringLiteral("Русский")};

    const QStringList files = appQmlFiles();
    QVERIFY(files.size() > 40);
    QStringList offenders;
    for (const QString &file : files) {
        const auto literals = stringLiterals(readAll(file));
        for (const StringLiteral &literal : literals) {
            if (!literal.translated && hasCyrillic(literal.text) && !allowed.contains(literal.text))
                offenders << QStringLiteral("%1:%2: %3").arg(file).arg(literal.line).arg(literal.text);
        }
    }
    QVERIFY2(offenders.isEmpty(), qPrintable(offenders.join(u'\n')));
}

void I18nTest::qmlShowsNoOldProductName() {
    QStringList offenders;
    const QStringList files = appQmlFiles();
    for (const QString &file : files) {
        const auto literals = stringLiterals(readAll(file));
        for (const StringLiteral &literal : literals) {
            if (literal.text.contains(QStringLiteral("meety"), Qt::CaseInsensitive))
                offenders << QStringLiteral("%1:%2: %3").arg(file).arg(literal.line).arg(literal.text);
        }
    }
    QVERIFY2(offenders.isEmpty(), qPrintable(offenders.join(u'\n')));
}

// With a format string these use the C locale whatever the language is.
void I18nTest::qmlDatesDoNotBypassTheLocale() {
    const QStringList files = appQmlFiles();
    for (const QString &file : files) {
        const QString source = readAll(file);
        QVERIFY2(!source.contains(QStringLiteral("Qt.formatDateTime(")), qPrintable(file));
        QVERIFY2(!source.contains(QStringLiteral("Qt.formatDate(")), qPrintable(file));
    }
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    I18nTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_i18n.moc"
