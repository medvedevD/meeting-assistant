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
//
// The store is then driven to "empty" so the screen renders its first-run body.
// That body is where the second loop lived (`font.letterSpacing` reading
// `font.pixelSize`, which re-notifies the whole font group), and it is
// unreachable while the store sits in "loading".

#include <QCoreApplication>
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
    void welcomeScreenReceivesStoreAndRendersFirstRun();
};

void AppShellWiringTest::welcomeScreenReceivesStoreAndRendersFirstRun() {
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

    QObject *welcome = nullptr;
    QObject *store = nullptr;
    if (shell) {
        welcome =
            findByClassPrefix(shell.get(), QStringLiteral("WelcomeScreen"));
        if (welcome)
            store = welcome->property("store").value<QObject *>();
        // A store left in "loading" only ever renders the busy placeholder, so
        // the first-run body — and any binding loop inside it — would stay out
        // of reach. Drive the list to "empty" to instantiate it.
        if (store)
            store->setProperty("status", QStringLiteral("empty"));
        QCoreApplication::processEvents();
    }

    qInstallMessageHandler(g_previous);
    g_previous = nullptr;

    QVERIFY2(shell != nullptr, qPrintable(component.errorString()));
    QVERIFY2(welcome != nullptr,
             "StackView did not create its initialItem (WelcomeScreen)");

    // The contract the self-binding broke: the screen must hold AppShell's
    // MeetingStore, not an undefined self-binding.
    QVERIFY2(welcome->property("store").isValid(),
             "WelcomeScreen has no `store` property — the screen's API changed");
    QVERIFY2(store != nullptr,
             "WelcomeScreen.store is unset: AppShell never passed its "
             "MeetingStore (bare `store:` self-binding?)");

    // Proves the first-run body above was actually reached, so the loop check
    // below covers it rather than passing vacuously on the placeholder.
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("firstRun"));

    const QStringList loops = g_messages.filter(QStringLiteral("Binding loop"));
    QVERIFY2(loops.isEmpty(), qPrintable(loops.join(QLatin1Char('\n'))));
}

QTEST_MAIN(AppShellWiringTest)

#include "tst_app_shell_wiring.moc"
