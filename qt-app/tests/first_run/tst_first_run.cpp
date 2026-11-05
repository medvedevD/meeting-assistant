#include "AppPreferences.h"
#include "EmbeddedQmldirGuard.h"

#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QTemporaryDir>
#include <QtTest>

#include <memory>
#include <utility>

namespace {

void collectTexts(const QQuickItem *item, QList<const QQuickItem *> &out) {
    for (const QQuickItem *child : item->childItems()) {
        if (child->metaObject()->indexOfProperty("renderType") >= 0)
            out.append(child);
        collectTexts(child, out);
    }
}

enum class Drawn { Upright, ThroughALayer, Transformed };

// What reaches `text` first on its way up the tree: a layer, which draws it flat
// before anything can turn it, or a rotation or scale, which applies to it live.
Drawn howDrawn(const QQuickItem *text) {
    for (const QQuickItem *item = text; item; item = item->parentItem()) {
        const auto *layer = item->property("layer").value<QObject *>();
        if (layer && layer->property("enabled").toBool())
            return Drawn::ThroughALayer;
        if (item->rotation() != 0 || item->scale() != 1)
            return Drawn::Transformed;
    }
    return Drawn::Upright;
}

} // namespace

class FakeMeetingStore final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString status READ status WRITE setStatus NOTIFY statusChanged)
    Q_PROPERTY(QVariantList meetings READ meetings WRITE setMeetings NOTIFY
                   meetingsChanged)

public:
    QString status() const { return m_status; }
    QVariantList meetings() const { return m_meetings; }

    void setStatus(const QString &status) {
        if (m_status == status)
            return;
        m_status = status;
        emit statusChanged();
    }

    void setMeetings(const QVariantList &meetings) {
        if (m_meetings == meetings)
            return;
        m_meetings = meetings;
        emit meetingsChanged();
    }

signals:
    void statusChanged();
    void meetingsChanged();

private:
    QString m_status = QStringLiteral("loading");
    QVariantList m_meetings;
};

class FakePreferences final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool onboardingCompleted READ onboardingCompleted NOTIFY
                   onboardingCompletedChanged)
    Q_PROPERTY(bool firstRunPreviewEnabled READ firstRunPreviewEnabled CONSTANT)

public:
    explicit FakePreferences(bool completed = false, bool preview = false)
        : m_completed(completed), m_preview(preview) {}

    bool onboardingCompleted() const { return m_completed; }
    bool firstRunPreviewEnabled() const { return m_preview; }
    int completionCalls() const { return m_completionCalls; }

    Q_INVOKABLE void completeOnboarding() {
        ++m_completionCalls;
        if (m_preview || m_completed)
            return;
        m_completed = true;
        emit onboardingCompletedChanged();
    }

signals:
    void onboardingCompletedChanged();

private:
    bool m_completed = false;
    bool m_preview = false;
    int m_completionCalls = 0;
};

class FirstRunTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void preferencesPersistCompletion();
    void previewDoesNotPersistCompletion();
    void previewEnvironmentRequiresValueOne();
    void freshEmptyProfileShowsOnboarding();
    void loadingDoesNotFlashOnboarding();
    void existingProfileCompletesOnboarding();
    void deletingLastMeetingDoesNotRestoreOnboarding();
    void previewForcesOnboarding();
    void rotatedCardDrawsItsTextFlatOnMacOS();

private:
    std::unique_ptr<QObject> createWelcome(FakeMeetingStore &store,
                                           FakePreferences &preferences);
};

std::unique_ptr<QObject>
FirstRunTest::createWelcome(FakeMeetingStore &store,
                            FakePreferences &preferences) {
    auto *engine = new QQmlEngine(this);
    engine->addImportPath(QStringLiteral(MA_FIRST_RUN_QML_IMPORT_DIR));

    QQmlComponent component(
        engine,
        QUrl::fromLocalFile(QStringLiteral(MA_WELCOME_QML_FILE)));
    if (!component.isReady()) {
        qWarning().noquote() << component.errorString();
        return {};
    }

    QVariantMap properties{
        {QStringLiteral("store"),
         QVariant::fromValue(static_cast<QObject *>(&store))},
        {QStringLiteral("preferences"),
         QVariant::fromValue(static_cast<QObject *>(&preferences))},
    };
    QObject *object = component.createWithInitialProperties(properties);
    if (!object)
        qWarning().noquote() << component.errorString();

    return std::unique_ptr<QObject>(object);
}

void FirstRunTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_FIRST_RUN_QML_IMPORT_DIR));
}

void FirstRunTest::preferencesPersistCompletion() {
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString path = dir.filePath(QStringLiteral("ui.ini"));

    {
        AppPreferences preferences(path, false);
        QVERIFY(!preferences.onboardingCompleted());
        preferences.completeOnboarding();
        QVERIFY(preferences.onboardingCompleted());
    }

    AppPreferences reloaded(path, false);
    QVERIFY(reloaded.onboardingCompleted());
}

void FirstRunTest::previewDoesNotPersistCompletion() {
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString path = dir.filePath(QStringLiteral("ui.ini"));

    {
        AppPreferences preview(path, true);
        preview.completeOnboarding();
        QVERIFY(!preview.onboardingCompleted());
    }

    AppPreferences reloaded(path, false);
    QVERIFY(!reloaded.onboardingCompleted());
}

void FirstRunTest::previewEnvironmentRequiresValueOne() {
    const QByteArray previous = qgetenv("FIRST_RUN");

    qunsetenv("FIRST_RUN");
    QVERIFY(!AppPreferences::firstRunPreviewFromEnvironment());
    qputenv("FIRST_RUN", "0");
    QVERIFY(!AppPreferences::firstRunPreviewFromEnvironment());
    qputenv("FIRST_RUN", "1");
    QVERIFY(AppPreferences::firstRunPreviewFromEnvironment());

    if (previous.isNull())
        qunsetenv("FIRST_RUN");
    else
        qputenv("FIRST_RUN", previous);
}

void FirstRunTest::freshEmptyProfileShowsOnboarding() {
    FakeMeetingStore store;
    store.setStatus(QStringLiteral("empty"));
    FakePreferences preferences;

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("firstRun"));
}

void FirstRunTest::loadingDoesNotFlashOnboarding() {
    FakeMeetingStore store;
    FakePreferences preferences;

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("loading"));
}

void FirstRunTest::existingProfileCompletesOnboarding() {
    FakeMeetingStore store;
    store.setMeetings(
        {QVariantMap{{QStringLiteral("id"), QStringLiteral("meeting-1")}}});
    store.setStatus(QStringLiteral("success"));
    FakePreferences preferences;

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QTRY_VERIFY(preferences.onboardingCompleted());
    QCOMPARE(preferences.completionCalls(), 1);
    QCOMPARE(welcome->property("viewState").toString(), QStringLiteral("empty"));
}

void FirstRunTest::deletingLastMeetingDoesNotRestoreOnboarding() {
    FakeMeetingStore store;
    store.setMeetings(
        {QVariantMap{{QStringLiteral("id"), QStringLiteral("meeting-1")}}});
    store.setStatus(QStringLiteral("success"));
    FakePreferences preferences;

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QTRY_VERIFY(preferences.onboardingCompleted());

    store.setStatus(QStringLiteral("loading"));
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("loading"));
    store.setMeetings({});
    store.setStatus(QStringLiteral("empty"));
    QCOMPARE(welcome->property("viewState").toString(), QStringLiteral("empty"));
}

void FirstRunTest::previewForcesOnboarding() {
    FakeMeetingStore store;
    store.setMeetings(
        {QVariantMap{{QStringLiteral("id"), QStringLiteral("meeting-1")}}});
    store.setStatus(QStringLiteral("success"));
    FakePreferences preferences(true, true);

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("firstRun"));
    QCOMPARE(preferences.completionCalls(), 0);
}

// Native text can't follow a rotation: each glyph stays upright and the line comes
// out stair-stepped. On macOS, where the card's serif text is native, a layer has to
// draw the text flat first. Other systems keep live text, which rotates cleanly.
void FirstRunTest::rotatedCardDrawsItsTextFlatOnMacOS() {
    FakeMeetingStore store;
    store.setStatus(QStringLiteral("empty"));
    FakePreferences preferences;

    const auto welcome = createWelcome(store, preferences);
    QVERIFY(welcome);
    QCOMPARE(welcome->property("viewState").toString(),
             QStringLiteral("firstRun"));

    const auto *root = qobject_cast<const QQuickItem *>(welcome.get());
    QVERIFY(root != nullptr);
    QList<const QQuickItem *> texts;
    collectTexts(root, texts);

#ifdef Q_OS_MACOS
    const Drawn expected = Drawn::ThroughALayer;
#else
    const Drawn expected = Drawn::Transformed;
#endif
    int onCard = 0;
    for (const QQuickItem *text : std::as_const(texts)) {
        const Drawn drawn = howDrawn(text);
        if (drawn == Drawn::Upright)
            continue;
        ++onCard;
        QVERIFY2(drawn == expected, qPrintable(text->property("text").toString()));
    }
    // Stops a screen that never built the card from passing.
    QVERIFY2(onCard >= 8, qPrintable(QString::number(onCard)));
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQuickStyle::setStyle(QStringLiteral("Fusion"));
    FirstRunTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_first_run.moc"
