#include "EmbeddedQmldirGuard.h"

#include <QFont>
#include <QGlyphRun>
#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QRawFont>
#include <QRegularExpression>
#include <QTextLayout>
#include <QtTest>

#include <algorithm>
#include <memory>
#include <utility>

// When a font lacks a letter, Qt draws that letter with another font and reports
// nothing: the Text's font.family still names the font it asked for. So these
// tests lay the text out with the app's real fonts and ask which fonts the glyphs
// came from.
//
// This is what broke: Newsreader has no Cyrillic, so a Russian line came out in a
// system sans with its digits and Latin words in Newsreader.

namespace {

const auto kNewsreader = QStringLiteral("Newsreader");
const auto kSourceSerif = QStringLiteral("Source Serif 4");

// Text.QtRendering and Text.NativeRendering.
constexpr int kQtRendering = 0;
constexpr int kNativeRendering = 1;

// A meeting with what real minutes contain: Latin words and digits inside Russian
// lines, emphasis, a quote, and table cells holding no Cyrillic at all.
const auto kRussianMinutes = QStringLiteral(
    "# Подготовка релиза версии 2.0\n\n"
    "## Краткое резюме\n"
    "Экспорт в PDF готов, осталось проверить сборку для Windows 11.\n\n"
    "### Готовность релиза 2.0\n"
    "- Мастер сокращён до 3 шагов, **ключ API** теперь необязателен.\n"
    "- Для Ollama нужна подсказка о *локальной* модели.\n\n"
    "1. Обновить README\n\n"
    "> Если настройка занимает больше минуты, человек уходит.\n\n"
    "| Действие | Срок |\n"
    "| --- | --- |\n"
    "| PDF | Q3 |\n");

const auto kEnglishMinutes = QStringLiteral(
    "# Preparing the 2.0 release\n\n"
    "## Summary\n"
    "PDF export is ready; the Windows 11 build still needs checking.\n\n"
    "### Release 2.0 readiness\n"
    "- The wizard is down to 3 steps, and an **API key** is now optional.\n"
    "- Ollama needs a hint about the *local* model.\n\n"
    "1. Update the README\n\n"
    "> If setup takes more than a minute, people leave.\n\n"
    "| Action | Due |\n"
    "| --- | --- |\n"
    "| PDF | Q3 |\n");

// The fonts Qt draws `text` with. A missing glyph shows up as a second family.
QStringList drawnFamilies(const QFont &font, const QString &text) {
    QTextLayout layout(text, font);
    layout.beginLayout();
    QTextLine line = layout.createLine();
    line.setLineWidth(100000);
    layout.endLayout();

    QStringList families;
    const QList<QGlyphRun> runs = layout.glyphRuns();
    for (const QGlyphRun &run : runs) {
        const QString family = run.rawFont().familyName();
        if (!families.contains(family))
            families.append(family);
    }
    return families;
}

void collectTexts(const QQuickItem *item, QList<const QQuickItem *> &out) {
    for (const QQuickItem *child : item->childItems()) {
        if (!child->property("text").toString().isEmpty()
            && child->property("font").canConvert<QFont>()) {
            out.append(child);
        }
        collectTexts(child, out);
    }
}

// Asserts that every serif Text in the document draws all its letters from
// `family`. Text set in the interface or code font is not serif text; checking
// the serif count stops a document that renders nothing from passing.
void verifyDrawnIn(const QObject *theme, const QObject *root, const QString &family) {
    const QString uiFont = theme->property("fontUi").toString();
    const QString monoFont = theme->property("fontMono").toString();

    const auto *item = qobject_cast<const QQuickItem *>(root);
    QVERIFY(item != nullptr);
    QList<const QQuickItem *> texts;
    collectTexts(item, texts);

    int serifTexts = 0;
    for (const QQuickItem *text : std::as_const(texts)) {
        QFont font = text->property("font").value<QFont>();
        if (font.family() == uiFont || font.family() == monoFont)
            continue;
        ++serifTexts;

        const QString plain = text->property("text").toString().remove(
            QRegularExpression(QStringLiteral("<[^>]+>")));
        // StyledText sets <b> and <i> runs inside the same Text.
        for (const bool emphasised : { false, true }) {
            QFont variant = font;
            if (emphasised) {
                variant.setBold(true);
                variant.setItalic(true);
            }
            const QStringList families = drawnFamilies(variant, plain);
            // startsWith: Windows can report an instance as "<family> SemiBold".
            QVERIFY2(!families.isEmpty() && std::all_of(families.cbegin(), families.cend(),
                                                        [&](const QString &name) {
                                                            return name.startsWith(family);
                                                        }),
                     qPrintable(QStringLiteral("'%1' is drawn with: %2")
                                    .arg(plain, families.join(QStringLiteral(", ")))));
        }

#ifdef Q_OS_MACOS
        const int expectedRenderType = family == kSourceSerif ? kNativeRendering : kQtRendering;
#else
        const int expectedRenderType = kQtRendering;
#endif
        QCOMPARE(text->property("renderType").toInt(), expectedRenderType);
    }
    QVERIFY2(serifTexts >= 8, qPrintable(QString::number(serifTexts)));
}

} // namespace

class SerifTypefaceTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void init();
    void cleanup();

    void russianMinutesDrawInOneSerif();
    void russianStructuredMinutesDrawInOneSerif();
    void englishMinutesKeepNewsreader();
    void englishStructuredMinutesKeepNewsreaderUnderRussianSectionTitles();
    void serifForFollowsTheText();

private:
    QString serifFor(const QString &text) const;

    std::unique_ptr<QQmlEngine> m_engine;
    std::unique_ptr<QQmlComponent> m_component;
    std::unique_ptr<QObject> m_document;
    QObject *m_theme = nullptr;
};

void SerifTypefaceTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_SERIF_QML_IMPORT_DIR));
}

void SerifTypefaceTest::init() {
    m_engine = std::make_unique<QQmlEngine>();
    m_engine->addImportPath(QStringLiteral(MA_SERIF_QML_IMPORT_DIR));

    m_component = std::make_unique<QQmlComponent>(m_engine.get());
    m_component->loadUrl(QUrl::fromLocalFile(QStringLiteral(MA_SERIF_QML_FILE)));
    QVERIFY2(m_component->isReady(), qPrintable(m_component->errorString()));
    m_document.reset(m_component->create());
    QVERIFY2(m_document != nullptr, qPrintable(m_component->errorString()));

    m_theme = m_engine->singletonInstance<QObject *>("MeetingAssistant", "Theme");
    QVERIFY(m_theme != nullptr);
}

void SerifTypefaceTest::cleanup() {
    m_document.reset();
    m_component.reset();
    m_engine.reset();
    m_theme = nullptr;
}

QString SerifTypefaceTest::serifFor(const QString &text) const {
    QVariant family;
    const bool invoked = QMetaObject::invokeMethod(m_theme, "serifFor", Q_RETURN_ARG(QVariant, family),
                                                   Q_ARG(QVariant, QVariant(text)));
    return invoked ? family.toString() : QString();
}

void SerifTypefaceTest::russianMinutesDrawInOneSerif() {
    m_document->setProperty("markdown", kRussianMinutes);

    verifyDrawnIn(m_theme, m_document.get(), kSourceSerif);
}

void SerifTypefaceTest::russianStructuredMinutesDrawInOneSerif() {
    QVariantMap topic;
    topic.insert(QStringLiteral("title"), QStringLiteral("Готовность релиза 2.0"));
    topic.insert(QStringLiteral("bullets"),
                 QVariantList{ QStringLiteral("Экспорт в PDF прошёл проверку на 3 ОС."),
                               QStringLiteral("Мастер сокращён до 3 шагов.") });
    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Проверить установщик на Windows 11"));
    action.insert(QStringLiteral("owner"), QStringLiteral("Maria"));
    action.insert(QStringLiteral("due"), QStringLiteral("Q3"));

    QVariantMap structured;
    structured.insert(QStringLiteral("title"), QStringLiteral("Подготовка релиза версии 2.0"));
    structured.insert(QStringLiteral("summary"), QVariantList{ QStringLiteral("Выпуск версии 2.0.") });
    structured.insert(QStringLiteral("topics"), QVariantList{ topic });
    structured.insert(QStringLiteral("actions"), QVariantList{ action });
    m_document->setProperty("structured", structured);

    verifyDrawnIn(m_theme, m_document.get(), kSourceSerif);
}

void SerifTypefaceTest::englishMinutesKeepNewsreader() {
    m_document->setProperty("markdown", kEnglishMinutes);

    verifyDrawnIn(m_theme, m_document.get(), kNewsreader);
}

// With no translator loaded the section titles below are Russian, as they are for
// English minutes in the Russian interface. They must not change the body's font.
void SerifTypefaceTest::englishStructuredMinutesKeepNewsreaderUnderRussianSectionTitles() {
    QVariantMap topic;
    topic.insert(QStringLiteral("title"), QStringLiteral("Release 2.0 readiness"));
    topic.insert(QStringLiteral("bullets"),
                 QVariantList{ QStringLiteral("PDF export passed on 3 systems."),
                               QStringLiteral("The wizard is down to 3 steps.") });
    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Check the installer on Windows 11"));
    action.insert(QStringLiteral("owner"), QStringLiteral("Maria"));
    action.insert(QStringLiteral("due"), QStringLiteral("Q3"));

    QVariantMap structured;
    structured.insert(QStringLiteral("title"), QStringLiteral("Preparing the 2.0 release"));
    structured.insert(QStringLiteral("summary"), QVariantList{ QStringLiteral("Version 2.0 ships.") });
    structured.insert(QStringLiteral("topics"), QVariantList{ topic });
    structured.insert(QStringLiteral("actions"), QVariantList{ action });
    m_document->setProperty("structured", structured);

    verifyDrawnIn(m_theme, m_document.get(), kNewsreader);
}

void SerifTypefaceTest::serifForFollowsTheText() {
    QCOMPARE(serifFor(QStringLiteral("Подготовка релиза 2.0")), kSourceSerif);
    QCOMPARE(serifFor(QStringLiteral("Release 2.0 и план")), kSourceSerif);
    QCOMPARE(serifFor(QStringLiteral("Preparing the 2.0 release")), kNewsreader);
    QCOMPARE(serifFor(QStringLiteral("Meeting Assistant")), kNewsreader);
    QCOMPARE(serifFor(QStringLiteral("00:14:32")), kNewsreader);
    QCOMPARE(serifFor(QString()), kNewsreader);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    SerifTypefaceTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_serif_typeface.moc"
