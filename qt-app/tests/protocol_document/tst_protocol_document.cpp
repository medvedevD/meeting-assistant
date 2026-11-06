#include "EmbeddedQmldirGuard.h"

#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickRenderControl>
#include <QQuickWindow>
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
    void spacesBlocksApart();
    void alignsTableColumns_data();
    void alignsTableColumns();
};

// Gives a document a width and lays it out. Layouts only arrange their items in a
// window's polish pass; QQuickRenderControl runs that pass directly, so real
// geometry can be read with no render loop, GPU, or exposed window.
class LaidOutDocument {
public:
    LaidOutDocument(QQuickItem *document, qreal width) : m_window(&m_control) {
        document->setParentItem(m_window.contentItem());
        document->setWidth(width);
        // Wrapping text changes implicit sizes, which the next pass picks up.
        for (int pass = 0; pass < 5; ++pass)
            m_control.polishItems();
    }

private:
    QQuickRenderControl m_control;
    QQuickWindow m_window;
};

// The document's blocks, in visual order. The Repeater is also a child item.
static QList<QQuickItem *> documentBlocks(const QQuickItem *document) {
    QList<QQuickItem *> blocks;
    for (QQuickItem *child : document->childItems()) {
        if (child->inherits("QQuickLoader"))
            blocks.append(child);
    }
    return blocks;
}

// The cells of a table, one list per row, with the header first.
static void collectTableRows(const QQuickItem *item, QList<QList<QQuickItem *>> &rows) {
    for (QQuickItem *child : item->childItems()) {
        if (!child->inherits("QQuickRowLayout")) {
            collectTableRows(child, rows);
            continue;
        }
        QList<QQuickItem *> cells;
        for (QQuickItem *cell : child->childItems()) {
            if (cell->inherits("QQuickText"))
                cells.append(cell);
        }
        rows.append(cells);
    }
}

static qreal leftEdge(const QQuickItem *cell, const QQuickItem *document) {
    return cell->mapToItem(document, QPointF(0, 0)).x();
}

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

// Each block's Layout.topMargin must reach the layout that positions the block.
void ProtocolDocumentTest::spacesBlocksApart() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    root->setProperty("markdown", QStringLiteral(
        "# Title\n\n"
        "#### Date: today\n\n"
        "## Section\n\n"
        "Paragraph\n\n"
        "- Item\n\n"
        "### Subsection\n\n"
        "1. First\n\n"
        "> Quote\n\n"
        "```\ncode\n```\n\n"
        "---\n\n"
        "#### Detail\n\n"
        "A | B\n--- | ---\n1 | 2\n"));

    auto *document = qobject_cast<QQuickItem *>(root.get());
    QVERIFY(document != nullptr);
    LaidOutDocument laidOut(document, 720);

    const QVariantList model = root->property("blocks").toList();
    const QList<QQuickItem *> blocks = documentBlocks(document);
    QCOMPARE(blocks.size(), model.size());

    // The screen sets the space above the document, so the first block adds none.
    QCOMPARE(blocks.first()->y(), 0.0);

    QStringList gaps;
    for (qsizetype i = 1; i < blocks.size(); ++i) {
        const qreal gap = blocks.at(i)->y() - (blocks.at(i - 1)->y() + blocks.at(i - 1)->height());
        gaps.append(model.at(i).toMap().value(QStringLiteral("type")).toString()
                    + QLatin1Char(':') + QString::number(qRound(gap)));
    }
    QCOMPARE(gaps, QStringList({
        QStringLiteral("meta:8"),
        QStringLiteral("h2:32"),
        QStringLiteral("p:14"),
        QStringLiteral("ul:8"),
        QStringLiteral("h3:22"),
        QStringLiteral("ol:8"),
        QStringLiteral("quote:16"),
        QStringLiteral("code:16"),
        QStringLiteral("hr:22"),
        QStringLiteral("h4:16"),
        QStringLiteral("table:12"),
    }));
}

void ProtocolDocumentTest::alignsTableColumns_data() {
    QVariantMap action;
    action.insert(QStringLiteral("title"), QStringLiteral("Подготовить сборку к релизу"));
    action.insert(QStringLiteral("owner"), QStringLiteral("Дима"));
    action.insert(QStringLiteral("due"), QStringLiteral("пятница"));
    QVariantMap structured;
    structured.insert(QStringLiteral("actions"), QVariantList { action });

    QTest::addColumn<QVariantMap>("structured");
    QTest::addColumn<QString>("markdown");
    QTest::addColumn<qreal>("width");

    QTest::newRow("action table") << structured << QString() << 720.0;
    QTest::newRow("action table, narrow") << structured << QString() << 300.0;
    QTest::newRow("one column") << QVariantMap()
        << QStringLiteral("| A |\n|---|\n| 1 |\n") << 720.0;
    QTest::newRow("two columns") << QVariantMap()
        << QStringLiteral("A | B\n--- | ---\n1 | 2\n") << 720.0;
    QTest::newRow("five columns") << QVariantMap()
        << QStringLiteral("A | B | C | D | E\n--- | --- | --- | --- | ---\n1 | 2 | 3 | 4 | 5\n") << 720.0;
    QTest::newRow("too many columns to fit") << QVariantMap()
        << QStringLiteral("A | B | C | D | E | F | G | H\n"
                          "--- | --- | --- | --- | --- | --- | --- | ---\n"
                          "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8\n") << 720.0;
    QTest::newRow("row shorter than the header") << QVariantMap()
        << QStringLiteral("A | B | C\n--- | --- | ---\n1 | 2 |\n4 | 5 | 6\n") << 720.0;
    QTest::newRow("row longer than the header") << QVariantMap()
        << QStringLiteral("A | B\n--- | ---\n1 | 2 | 3\n4 | 5\n") << 720.0;
}

// The header and each body row are separate layouts, so nothing but matching
// sizes keeps a column straight.
void ProtocolDocumentTest::alignsTableColumns() {
    QFETCH(QVariantMap, structured);
    QFETCH(QString, markdown);
    QFETCH(qreal, width);

    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_PROTOCOL_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createDocument(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    root->setProperty("markdown", markdown);
    root->setProperty("structured", structured);

    auto *document = qobject_cast<QQuickItem *>(root.get());
    QVERIFY(document != nullptr);
    LaidOutDocument laidOut(document, width);

    QList<QList<QQuickItem *>> rows;
    collectTableRows(document, rows);
    QVERIFY2(rows.size() >= 2, "expected a header and at least one body row");

    const QList<QQuickItem *> &header = rows.first();
    QVERIFY(!header.isEmpty());
    for (const QList<QQuickItem *> &row : rows) {
        QCOMPARE(row.size(), header.size());
        for (qsizetype column = 0; column < row.size(); ++column) {
            QCOMPARE(leftEdge(row.at(column), document), leftEdge(header.at(column), document));
            QCOMPARE(row.at(column)->width(), header.at(column)->width());
        }
    }

    // The columns stay inside the document, and none is squeezed below the rest.
    const QQuickItem *lastColumn = header.last();
    QVERIFY(leftEdge(lastColumn, document) + lastColumn->width() <= width + 0.5);
    QVERIFY(header.first()->width() >= lastColumn->width() - 0.5);
    QVERIFY(lastColumn->width() > 0);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    ProtocolDocumentTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_protocol_document.moc"
