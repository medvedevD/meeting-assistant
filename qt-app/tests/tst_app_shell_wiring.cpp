// AppShell → WelcomeScreen wiring regression.
//
// AppShell instantiates WelcomeScreen declaratively inside a `Component`, and a
// bare `store: store` there does NOT reach `MeetingStore { id: store }`: the
// component's own `property var store` shadows the outer id, so the property
// binds to itself. Qt reports "Binding loop detected for property store" and
// leaves the screen with an undefined store, which then silently degrades —
// `viewState` can never be "loading" or "firstRun", so onboarding never shows,
// and `reconcileExistingProfile()` returns early forever. `Connections` on the
// undefined target adds "no signal of the target matches onStatusChanged" and
// "Unable to assign [undefined] to QObject*". AppShell already carries
// `shellRef` as the escape hatch for exactly this trap on `shell`; `storeRef`
// is its counterpart.
//
// The real AppShell.qml is loaded here rather than a copy, so the guard tracks
// the shipped file. The six pushable screens are stubbed (they only need to
// resolve — each lives inside a lazy Component the test never instantiates),
// and `api.configured` stays false so the store never issues a request.

#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QUrl>
#include <QtTest>

#include <memory>

namespace {

// The engine-wide context properties main.cpp installs.
class FakeApi final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool configured READ configured NOTIFY configuredChanged)

public:
    bool configured() const { return false; }

signals:
    void configuredChanged();
};

class FakePreferences final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool onboardingCompleted READ onboardingCompleted NOTIFY
                   onboardingCompletedChanged)
    Q_PROPERTY(bool firstRunPreviewEnabled READ firstRunPreviewEnabled CONSTANT)

public:
    bool onboardingCompleted() const { return false; }
    bool firstRunPreviewEnabled() const { return false; }
    Q_INVOKABLE void completeOnboarding() {}

signals:
    void onboardingCompletedChanged();
};

QStringList g_messages;
QtMessageHandler g_previous = nullptr;

void captureMessages(QtMsgType type, const QMessageLogContext &ctx,
                     const QString &msg) {
    g_messages << msg;
    if (g_previous)
        g_previous(type, ctx, msg);
}

// QML-declared types get a generated class name ("WelcomeScreen_QMLTYPE_42"),
// so match on the prefix rather than an exact name.
QObject *findByClassPrefix(QObject *root, const QString &prefix) {
    if (QString::fromLatin1(root->metaObject()->className()).startsWith(prefix))
        return root;
    const QObjectList children = root->children();
    for (QObject *child : children)
        if (QObject *hit = findByClassPrefix(child, prefix))
            return hit;
    return nullptr;
}

} // namespace

class AppShellWiringTest : public QObject {
    Q_OBJECT

private slots:
    void welcomeScreenReceivesStore();
};

void AppShellWiringTest::welcomeScreenReceivesStore() {
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(MA_APP_SHELL_QML_IMPORT_DIR));

    FakeApi api;
    FakePreferences preferences;
    engine.rootContext()->setContextProperty(QStringLiteral("api"), &api);
    engine.rootContext()->setContextProperty(QStringLiteral("appPreferences"),
                                             &preferences);

    g_messages.clear();
    g_previous = qInstallMessageHandler(captureMessages);
    QQmlComponent component(
        &engine, QUrl::fromLocalFile(QStringLiteral(MA_APP_SHELL_QML_FILE)));
    const std::unique_ptr<QObject> shell(component.create());
    qInstallMessageHandler(g_previous);
    g_previous = nullptr;

    QVERIFY2(shell != nullptr, qPrintable(component.errorString()));

    // Fails on the pre-fix `store: store`.
    const QStringList loops = g_messages.filter(QStringLiteral("Binding loop"));
    QVERIFY2(loops.isEmpty(), qPrintable(loops.join(QLatin1Char('\n'))));

    QObject *welcome =
        findByClassPrefix(shell.get(), QStringLiteral("WelcomeScreen"));
    QVERIFY2(welcome != nullptr,
             "StackView did not create its initialItem (WelcomeScreen)");

    // The contract the binding loop broke: the screen must hold AppShell's
    // MeetingStore, not an undefined self-binding.
    const QVariant store = welcome->property("store");
    QVERIFY2(store.isValid(),
             "WelcomeScreen has no `store` property — the screen's API changed");
    QVERIFY2(store.value<QObject *>() != nullptr,
             "WelcomeScreen.store is unset: AppShell never passed its "
             "MeetingStore (bare `store:` self-binding?)");
}

QTEST_MAIN(AppShellWiringTest)

#include "tst_app_shell_wiring.moc"
