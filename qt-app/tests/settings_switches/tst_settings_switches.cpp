// Settings switches must save every change of `checked`, however it came.
//
// Smoke test of v0.1.0-alpha.14: switching "Echo cancellation" with VoiceOver
// moved the switch, but "Save" wrote the old value. Qt Quick carries out the
// accessibility toggle action by setting `checked` directly, without
// `toggled`, and the panels wrote the draft only in `onToggled`.
//
// The real RecordingPanel and WhisperPanel run on a stand-in settings screen
// (`scr`: the draft, `touch()`, `reseeded`), with a stand-in `api` that never
// answers, so the panels' requests stay pending.

#include "EmbeddedQmldirGuard.h"

#include <QAccessible>
#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QtTest>
#include <QtTest/qtestaccessible.h>

// Stand-in for the engine-wide `api` (ApiClient) that Request.qml calls.
class SilentApi : public QObject {
    Q_OBJECT
public:
    Q_INVOKABLE int get(const QString &) { return ++m_lastId; }
    Q_INVOKABLE int post(const QString &, const QVariantMap &) { return ++m_lastId; }
    Q_INVOKABLE int put(const QString &, const QVariantMap &) { return ++m_lastId; }
    Q_INVOKABLE int del(const QString &) { return ++m_lastId; }

signals:
    void requestSucceeded(int requestId, const QString &path, const QVariant &json);
    void requestFailed(int requestId, const QString &path, int httpStatus,
                       const QString &error, const QVariant &body);

private:
    int m_lastId = 0;
};

// The item named `name` anywhere under `root` in the visual tree, which is
// where a ScrollView's content ends up.
static QQuickItem *findItem(QQuickItem *root, const QString &name) {
    if (!root)
        return nullptr;
    if (root->objectName() == name)
        return root;
    for (QQuickItem *child : root->childItems()) {
        if (QQuickItem *found = findItem(child, name))
            return found;
    }
    return nullptr;
}

class SettingsSwitchesTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void cleanup();
    void everyChangeOfCheckedIsSaved_data();
    void everyChangeOfCheckedIsSaved();
    void openingThePanelsWritesNothing();

private:
    // Load both panels over `draft`; the window stays up until cleanup().
    QQuickWindow *showPanels(const QString &draft);
    QVariant draftValue(const QString &key);
    int rev();

    QQmlEngine *m_engine = nullptr;
    SilentApi m_api;
    std::unique_ptr<QObject> m_root;
};

void SettingsSwitchesTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_SETTINGS_SWITCHES_QML_IMPORT_DIR));
    // Accessibility events go to the test, not to a platform bridge.
    QTestAccessibility::initialize();
}

void SettingsSwitchesTest::cleanup() {
    m_root.reset();
    delete m_engine;
    m_engine = nullptr;
}

QQuickWindow *SettingsSwitchesTest::showPanels(const QString &draft) {
    m_engine = new QQmlEngine;
    m_engine->addImportPath(QStringLiteral(MA_SETTINGS_SWITCHES_QML_IMPORT_DIR));
    m_engine->rootContext()->setContextProperty(QStringLiteral("api"), &m_api);

    QQmlComponent component(m_engine);
    component.setData(QStringLiteral(R"qml(
        import QtQuick
        import QtQuick.Controls
        import MeetingAssistant

        ApplicationWindow {
            width: 900
            height: 3000
            visible: true

            function draftValue(key) {
                return key === "echo_cancel" ? scr.draft.recording.echo_cancel
                                             : scr.draft.transcriber.use_gpu
            }
            function rev() { return scr.rev }

            QtObject {
                id: scr
                property var draft: (%1)
                property int rev: 0
                function touch() { rev++ }
                signal reseeded()
            }

            Column {
                width: parent.width
                // Tall enough to show everything, so a click lands unscrolled.
                RecordingPanel { scr: scr; width: parent.width; height: contentHeight }
                WhisperPanel { scr: scr; width: parent.width; height: contentHeight }
            }
        }
    )qml").arg(draft).toUtf8(), QUrl());

    m_root.reset(component.create());
    if (!m_root) {
        qWarning().noquote() << component.errorString();
        return nullptr;
    }
    // The offscreen platform has no accessibility bridge, so accessibility
    // never turns on by itself. Until it does, Qt 6.7's controls have no
    // accessible role and Qt Quick gives them no accessible interface (6.11
    // gives one anyway). Announce it the way a screen reader starting up
    // would, so every control sets up its role.
    QAccessible::setActive(true);
    auto *window = qobject_cast<QQuickWindow *>(m_root.get());
    if (window && !QTest::qWaitForWindowExposed(window))
        return nullptr;
    return window;
}

QVariant SettingsSwitchesTest::draftValue(const QString &key) {
    QVariant value;
    QMetaObject::invokeMethod(m_root.get(), "draftValue", Q_RETURN_ARG(QVariant, value),
                              Q_ARG(QVariant, key));
    return value;
}

int SettingsSwitchesTest::rev() {
    QVariant value;
    QMetaObject::invokeMethod(m_root.get(), "rev", Q_RETURN_ARG(QVariant, value));
    return value.toInt();
}

void SettingsSwitchesTest::everyChangeOfCheckedIsSaved_data() {
    QTest::addColumn<QString>("switchName");
    QTest::addColumn<QString>("draftKey");
    QTest::addColumn<QString>("how");

    const QList<QPair<QString, QString>> switches = {
        {QStringLiteral("echoSwitch"), QStringLiteral("echo_cancel")},
        {QStringLiteral("gpuSwitch"), QStringLiteral("use_gpu")},
    };
    for (const auto &[switchName, draftKey] : switches) {
        for (const QString how : {QStringLiteral("mouse"), QStringLiteral("space key"),
                                  QAccessibleActionInterface::toggleAction()}) {
            QTest::addRow("%s by %s", qPrintable(switchName), qPrintable(how))
                << switchName << draftKey << how;
        }
    }
}

void SettingsSwitchesTest::everyChangeOfCheckedIsSaved() {
    QFETCH(QString, switchName);
    QFETCH(QString, draftKey);
    QFETCH(QString, how);

    QQuickWindow *window = showPanels(QStringLiteral(
        "{ recording: { source: 'mic', echo_cancel: false },"
        "  transcriber: { backend: 'metal', use_gpu: false }, paths: {} }"));
    QVERIFY(window);
    auto *control = findItem(window->contentItem(), switchName);
    QVERIFY(control);
    QVERIFY(control->isEnabled());
    QCOMPARE(control->property("checked").toBool(), false);
    const int revBefore = rev();

    if (how == QLatin1String("mouse")) {
        const QPoint centre =
            control->mapToScene(QPointF(control->width() / 2, control->height() / 2)).toPoint();
        QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, centre);
    } else if (how == QLatin1String("space key")) {
        control->forceActiveFocus();
        QTest::keyClick(window, Qt::Key_Space);
    } else {
        QAccessibleInterface *accessible = QAccessible::queryAccessibleInterface(control);
        QVERIFY(accessible);
        QAccessibleActionInterface *actions = accessible->actionInterface();
        QVERIFY(actions);
        QVERIFY2(actions->actionNames().contains(how),
                 qPrintable(actions->actionNames().join(", ")));
        actions->doAction(how);
    }

    QTRY_COMPARE(control->property("checked").toBool(), true);
    QCOMPARE(draftValue(draftKey), QVariant(true));
    QVERIFY(rev() > revBefore);
}

// The panels set their switches from the draft on load, and on every reseed.
// Reacting to each change of `checked` must not write that back: `use_gpu`
// unset means "on" and stays unset, and nothing counts as an edit.
void SettingsSwitchesTest::openingThePanelsWritesNothing() {
    QQuickWindow *window = showPanels(QStringLiteral(
        "{ recording: { source: 'mic', echo_cancel: true },"
        "  transcriber: { backend: 'metal' }, paths: {} }"));
    QVERIFY(window);
    for (const QString name : {QStringLiteral("echoSwitch"), QStringLiteral("gpuSwitch")}) {
        QQuickItem *control = findItem(window->contentItem(), name);
        QVERIFY2(control, qPrintable(name));
        QVERIFY2(control->property("checked").toBool(), qPrintable(name));
    }

    QCOMPARE(draftValue(QStringLiteral("echo_cancel")), QVariant(true));
    QVERIFY(!draftValue(QStringLiteral("use_gpu")).isValid());
    QCOMPARE(rev(), 0);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQuickStyle::setStyle(QStringLiteral("Fusion")); // as main.cpp does
    SettingsSwitchesTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_settings_switches.moc"
