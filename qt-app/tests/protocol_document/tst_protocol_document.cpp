#include "EmbeddedQmldirGuard.h"

#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QtTest>

class ProtocolDocumentTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void parsesEditorialMarkdownBlocks();
    void parsesDeepHeadingsWithoutVisibleHashes();
    void prefersStructuredProtocolBlocks();
    void hidesEmptyStructuredSections();
    void showsOtherSectionsInDocumentOrder();
    void otherSectionsAloneCountAsStructured();
    void escapesInlineHtml();
};

static QVariantMap otherSection(const QString &title, const QString &markdown, const QVariant &before) {
    QVariantMap section;
    section.insert(QStringLiteral("title"), title);
    section.insert(QStringLiteral("markdown"), markdown);
    section.insert(QStringLiteral("before"), before);
    return section;
}

// Every Text the document actually shows, in visual order.
static void collectTexts(const QQuickItem *item, QStringList &out) {
    for (const QQuickItem *child : item->childItems()) {
        const QString text = child->property("text").toString();
        if (!text.isEmpty())
            out.append(text);
        collectTexts(child, out);
    }
}

static std::unique_ptr<QObject> createDocument(QQmlEngine &engine, QQmlComponent &component) {
    component.loadUrl(QUrl::fromLocalFile(QStringLiteral(MA_PROTOCOL_QML_FILE)));

    if (!component.isReady())
        return nullptr;
    return std::unique_ptr<QObject>(component.create());
}

void ProtocolDocumentTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));
}

void ProtocolDocumentTest::parsesEditorialMarkdownBlocks() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    root->setProperty("markdown", QStringLiteral(
        "# Title\n\n"
        "> Decision **approved**\n"
        "> Follow-up\n\n"
        "```json\n"
        "{\"name\":\"<demo>\"}\n"
        "```\n\n"
        "Owner | Action\n"
        "--- | ---\n"
        "A\\|B | Ship it\n\n"
        "Paragraph with `code`.\n"));

    const QVariantList blocks = root->property("blocks").toList();
    QCOMPARE(blocks.size(), 5);
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h1"));
    QCOMPARE(blocks.at(1).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("quote"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("code"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("{\"name\":\"<demo>\"}"));
    QCOMPARE(blocks.at(3).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("table"));

    const QVariantList rows = blocks.at(3).toMap().value(QStringLiteral("rows")).toList();
    QCOMPARE(rows.size(), 1);
    QCOMPARE(rows.at(0).toList().at(0).toString(), QStringLiteral("A|B"));
    QCOMPARE(blocks.at(4).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("p"));
}

void ProtocolDocumentTest::parsesDeepHeadingsWithoutVisibleHashes() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    root->setProperty("markdown", QStringLiteral(
        "### **Протокол встречи**\n\n"
        "#### **Дата:** [Указать дату]\n"
        "#### **1.1. Журналы (логирование)**\n"
        "###### Deep heading ######\n"));

    const QVariantList blocks = root->property("blocks").toList();
    QCOMPARE(blocks.size(), 4);
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h1"));
    QCOMPARE(blocks.at(1).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("meta"));
    QCOMPARE(blocks.at(1).toMap().value(QStringLiteral("items")).toList().at(0).toString(),
             QStringLiteral("**Дата:** [Указать дату]"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h3"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("level")).toInt(), 4);
    QCOMPARE(blocks.at(3).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h4"));
    QCOMPARE(blocks.at(3).toMap().value(QStringLiteral("level")).toInt(), 6);
    QCOMPARE(blocks.at(3).toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("Deep heading"));
}

void ProtocolDocumentTest::prefersStructuredProtocolBlocks() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Подготовить сборку"));
    action.insert(QStringLiteral("owner"), QStringLiteral("Дима"));
    action.insert(QStringLiteral("due"), QStringLiteral("пятница"));

    QVariantMap topic;
    topic.insert(QStringLiteral("title"), QStringLiteral("Риски"));
    topic.insert(QStringLiteral("bullets"), QVariantList { QStringLiteral("Проверить интеграцию") });

    QVariantMap structured;
    structured.insert(QStringLiteral("title"), QStringLiteral("Структурный протокол"));
    structured.insert(QStringLiteral("summary"), QVariantList { QStringLiteral("Короткий итог") });
    structured.insert(QStringLiteral("topics"), QVariantList { topic });
    structured.insert(QStringLiteral("decisions"), QVariantList { QStringLiteral("Выпускать MVP") });
    structured.insert(QStringLiteral("actions"), QVariantList { action });
    structured.insert(QStringLiteral("open_questions"), QVariantList { QStringLiteral("Кто проведет демо?") });

    root->setProperty("markdown", QStringLiteral("# Markdown fallback"));
    root->setProperty("structured", structured);

    const QVariantList blocks = root->property("blocks").toList();
    QVERIFY(blocks.size() >= 10);
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h1"));
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("Структурный протокол"));
    QCOMPARE(blocks.at(1).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h2"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("p"));
    QCOMPARE(blocks.at(2).toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("Короткий итог"));

    bool sawActionsTable = false;
    for (const QVariant &block : blocks) {
        const QVariantMap map = block.toMap();
        if (map.value(QStringLiteral("type")).toString() == QStringLiteral("table")) {
            sawActionsTable = true;
            const QVariantList rows = map.value(QStringLiteral("rows")).toList();
            QCOMPARE(rows.at(0).toList().at(0).toString(), QStringLiteral("Подготовить сборку"));
            QCOMPARE(rows.at(0).toList().at(1).toString(), QStringLiteral("Дима"));
        }
    }
    QVERIFY(sawActionsTable);
}

void ProtocolDocumentTest::hidesEmptyStructuredSections() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Нет"));
    action.insert(QStringLiteral("owner"), QString());
    action.insert(QStringLiteral("due"), QString());

    QVariantMap structured;
    structured.insert(QStringLiteral("title"), QStringLiteral("Протокол"));
    structured.insert(QStringLiteral("summary"), QVariantList { QStringLiteral("Итог есть") });
    structured.insert(QStringLiteral("decisions"), QVariantList { QStringLiteral("Нет") });
    structured.insert(QStringLiteral("actions"), QVariantList { action });
    structured.insert(QStringLiteral("open_questions"), QVariantList { QStringLiteral("Не обсуждалось") });

    root->setProperty("structured", structured);

    const QVariantList blocks = root->property("blocks").toList();
    for (const QVariant &block : blocks) {
        const QVariantMap map = block.toMap();
        const QString text = map.value(QStringLiteral("text")).toString();
        QVERIFY(text != QStringLiteral("Решения"));
        QVERIFY(text != QStringLiteral("Дальнейшие действия"));
        QVERIFY(text != QStringLiteral("Открытые вопросы"));
        QVERIFY(map.value(QStringLiteral("type")).toString() != QStringLiteral("table"));
    }
}

void ProtocolDocumentTest::showsOtherSectionsInDocumentOrder() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Подготовить сборку"));
    action.insert(QStringLiteral("owner"), QStringLiteral("Дима"));

    QVariantMap structured;
    structured.insert(QStringLiteral("summary"), QVariantList { QStringLiteral("Короткий итог") });
    structured.insert(QStringLiteral("decisions"), QVariantList { QStringLiteral("Выпускать") });
    structured.insert(QStringLiteral("actions"), QVariantList { action });
    structured.insert(QStringLiteral("other_sections"), QVariantList {
        otherSection(QStringLiteral("Цель встречи"), QStringLiteral("Договориться о релизе."),
                     QStringLiteral("summary")),
        otherSection(QStringLiteral("Статус по участникам"),
                     QStringLiteral("### Артем\n- **Сделано:** тикет\n\n### Мария\n- План"),
                     QStringLiteral("decisions")),
        otherSection(QStringLiteral("Обратная связь"), QStringLiteral("Всё хорошо."), QVariant()),
    });

    root->setProperty("structured", structured);

    QStringList outline;
    const QVariantList blocks = root->property("blocks").toList();
    for (const QVariant &block : blocks) {
        const QVariantMap map = block.toMap();
        const QString type = map.value(QStringLiteral("type")).toString();
        outline.append(map.contains(QStringLiteral("text"))
                           ? type + QLatin1Char(':') + map.value(QStringLiteral("text")).toString()
                           : type);
    }
    QCOMPARE(outline, QStringList({
        QStringLiteral("h2:Цель встречи"),
        QStringLiteral("p:Договориться о релизе."),
        QStringLiteral("h2:Краткое резюме"),
        QStringLiteral("p:Короткий итог"),
        QStringLiteral("h2:Статус по участникам"),
        QStringLiteral("h3:Артем"),
        QStringLiteral("ul"),
        QStringLiteral("h3:Мария"),
        QStringLiteral("ul"),
        QStringLiteral("h2:Решения"),
        QStringLiteral("ul"),
        QStringLiteral("h2:Дальнейшие действия"),
        QStringLiteral("table"),
        QStringLiteral("h2:Обратная связь"),
        QStringLiteral("p:Всё хорошо."),
    }));

    const auto *item = qobject_cast<QQuickItem *>(root.get());
    QVERIFY(item != nullptr);
    QStringList shown;
    collectTexts(item, shown);
    QVERIFY2(shown.contains(QStringLiteral("СТАТУС ПО УЧАСТНИКАМ")), qPrintable(shown.join(u'|')));
    QVERIFY2(shown.contains(QStringLiteral("Артем")), qPrintable(shown.join(u'|')));
    QVERIFY2(shown.contains(QStringLiteral("<b>Сделано:</b> тикет")), qPrintable(shown.join(u'|')));
}

void ProtocolDocumentTest::otherSectionsAloneCountAsStructured() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QVariantMap structured;
    structured.insert(QStringLiteral("other_sections"), QVariantList {
        otherSection(QStringLiteral("Обратная связь"), QStringLiteral("Всё хорошо."), QVariant()),
    });

    root->setProperty("markdown", QStringLiteral("# Markdown fallback"));
    root->setProperty("structured", structured);

    const QVariantList blocks = root->property("blocks").toList();
    QCOMPARE(blocks.size(), 2);
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("type")).toString(), QStringLiteral("h2"));
    QCOMPARE(blocks.at(0).toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("Обратная связь"));
}

void ProtocolDocumentTest::escapesInlineHtml() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QVariant rendered;
    const bool invoked = QMetaObject::invokeMethod(
        root.get(),
        "_inline",
        Q_RETURN_ARG(QVariant, rendered),
        Q_ARG(QVariant, QVariant(QStringLiteral("Use **bold** `x<y` [link](https://example.test)"))));
    QVERIFY(invoked);

    const QString text = rendered.toString();
    QVERIFY(text.contains(QStringLiteral("<b>bold</b>")));
    QVERIFY(text.contains(QStringLiteral("x&lt;y")));
    QVERIFY(text.contains(QStringLiteral("<a href=\"https://example.test\">link</a>")));
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    ProtocolDocumentTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_protocol_document.moc"
