#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QtTest>

class ProtocolDocumentTest : public QObject {
    Q_OBJECT

private slots:
    void parsesEditorialMarkdownBlocks();
    void parsesDeepHeadingsWithoutVisibleHashes();
    void escapesInlineHtml();
};

static std::unique_ptr<QObject> createDocument(QQmlEngine &engine, QQmlComponent &component) {
    component.setData(R"qml(
        import QtQuick
        import MeetingAssistant
        ProtocolDocument {}
    )qml", QUrl());

    if (!component.isReady())
        return nullptr;
    return std::unique_ptr<QObject>(component.create());
}

void ProtocolDocumentTest::parsesEditorialMarkdownBlocks() {
    QQmlEngine engine;

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

void ProtocolDocumentTest::escapesInlineHtml() {
    QQmlEngine engine;

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
