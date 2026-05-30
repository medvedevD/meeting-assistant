#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QtTest>

class MeetyMenuTest : public QObject {
    Q_OBJECT

private slots:
    void longItemsExpandMenuWidth();
    void popupFromButtonOffsetsMenu();
};

static std::unique_ptr<QObject> createWindow(QQmlEngine &engine, QQmlComponent &component) {
    component.setData(R"qml(
        import QtQuick
        import QtQuick.Controls
        import MeetingAssistant

        ApplicationWindow {
            width: 700
            height: 400
            visible: true

            function openMenuFromAnchor() {
                menu.popupFromButton(anchor, 32, 8)
            }
            function expectedMenuX() {
                const host = Overlay.overlay ? Overlay.overlay : menu.parent
                return Math.max(8, host.width - menu.width - 32)
            }
            function expectedMenuY() {
                const host = Overlay.overlay ? Overlay.overlay : menu.parent
                const p = anchor.mapToItem(host, 0, 0)
                return Math.max(0, p.y + anchor.height + 8)
            }

            Item {
                id: page
                width: 700
                height: 400

                Item {
                    id: anchor
                    objectName: "anchor"
                    x: 620
                    y: 16
                    width: 32
                    height: 32
                }

                MeetyMenu {
                    id: menu
                    objectName: "menu"

                    MeetyMenuItem {
                        objectName: "menuItem"
                        text: qsTr("Удалить аудио (оставить транскрипт)")
                        iconName: "trash"
                        danger: true
                    }
                }
            }
        }
    )qml", QUrl());

    if (!component.isReady())
        return nullptr;
    return std::unique_ptr<QObject>(component.create());
}

void MeetyMenuTest::longItemsExpandMenuWidth() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_MENU_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createWindow(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QObject *menu = root->findChild<QObject *>(QStringLiteral("menu"));
    QVERIFY(menu != nullptr);
    QObject *menuItem = root->findChild<QObject *>(QStringLiteral("menuItem"));
    QVERIFY(menuItem != nullptr);

    QTRY_VERIFY_WITH_TIMEOUT(menuItem->property("implicitWidth").toReal() > 240.0, 1000);
    const bool opened = QMetaObject::invokeMethod(menu, "open");
    QVERIFY(opened);
    QTRY_VERIFY_WITH_TIMEOUT(menu->property("width").toReal() > 240.0, 1000);
}

void MeetyMenuTest::popupFromButtonOffsetsMenu() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_MENU_QML_IMPORT_DIR));

    QQmlComponent component(&engine);
    std::unique_ptr<QObject> root = createWindow(engine, component);
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QObject *menu = root->findChild<QObject *>(QStringLiteral("menu"));
    QVERIFY(menu != nullptr);

    const bool opened = QMetaObject::invokeMethod(root.get(), "openMenuFromAnchor");
    QVERIFY(opened);

    QVariant expectedX;
    QVERIFY(QMetaObject::invokeMethod(root.get(), "expectedMenuX",
                                      Q_RETURN_ARG(QVariant, expectedX)));
    QVariant expectedY;
    QVERIFY(QMetaObject::invokeMethod(root.get(), "expectedMenuY",
                                      Q_RETURN_ARG(QVariant, expectedY)));

    QTRY_COMPARE_WITH_TIMEOUT(menu->property("x").toReal(), expectedX.toReal(), 1000);
    QTRY_COMPARE_WITH_TIMEOUT(menu->property("y").toReal(), expectedY.toReal(), 1000);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    MeetyMenuTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_meety_menu.moc"
