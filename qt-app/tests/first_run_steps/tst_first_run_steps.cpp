// The first-run setup steps: microphone, speech recognition model, minutes.
// Also the notices a later launch shows on the home screen.
//
// The first-run screen used to list these three as text. A new user could not
// complete or check any of them there, and learned what was missing only after
// recording a meeting. Each step is now done in place, and this test drives the
// real FirstRunSteps with the real stores and ApiClient. Three things are not
// real:
//   - the sidecar: FakeSidecar answers the same HTTP routes, because a real one
//     would download a model and call an LLM provider;
//   - MicrophonePermission: the real one asks macOS;
//   - AppPreferences: the fake keeps "microphone tested" in memory.
//
// No translator is loaded, so visible text is the Russian source text.

#include "ApiClient.h"
#include "EmbeddedQmldirGuard.h"

#include <QGuiApplication>
#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QtTest>

#include <memory>

namespace {

class FakeSidecar final : public QObject {
    Q_OBJECT

public:
    FakeSidecar() {
        settings = QJsonObject{
            {QStringLiteral("minutes_setup_skipped"), false},
            {QStringLiteral("transcriber"),
             QJsonObject{{QStringLiteral("model_source"), QStringLiteral("managed")},
                         {QStringLiteral("model_id"), QJsonValue()}}},
            {QStringLiteral("llm"),
             QJsonObject{{QStringLiteral("active"), QStringLiteral("anthropic")},
                         {QStringLiteral("ollama"),
                          QJsonObject{{QStringLiteral("model"), QStringLiteral("llama3.1")},
                                      {QStringLiteral("has_key"), true}}}}},
            {QStringLiteral("secret_storage"),
             QJsonObject{{QStringLiteral("kind"), QStringLiteral("plaintext")},
                         {QStringLiteral("state"), QStringLiteral("ready")}}}};
    }

    bool listen() {
        connect(&m_server, &QTcpServer::newConnection, this, [this]() {
            QTcpSocket *sock = m_server.nextPendingConnection();
            connect(sock, &QTcpSocket::readyRead, this, [this, sock]() { onReadyRead(sock); });
            connect(sock, &QTcpSocket::disconnected, sock, &QObject::deleteLater);
        });
        return m_server.listen(QHostAddress::LocalHost, 0);
    }
    QString baseUrl() const {
        return QStringLiteral("http://127.0.0.1:%1").arg(m_server.serverPort());
    }

    // ── what the test sets ──────────────────────────────────────────────────
    QJsonObject settings;
    bool hasMicrophone = true;
    double micLevel = 0.0;
    // "" (no job) | "downloading" | "done" | "failed"
    QString installStatus;
    double installPercent = 0;
    // Error class the provider check fails with, "" when it passes.
    QString providerCheckFails;
    // A model was selected and its file is gone.
    bool modelFileMissing = false;
    // The passphrase vault is locked.
    bool keysLocked = false;
    // What the recovery pass repaired at this start.
    QJsonArray recovered;

    // ── what the test reads ─────────────────────────────────────────────────
    // Each request as "METHOD /path".
    QStringList requests;
    QHash<QString, QString> secrets;
    int count(const QString &request) const { return requests.count(request); }
    bool sawPrefix(const QString &prefix) const {
        for (const QString &r : requests)
            if (r.startsWith(prefix))
                return true;
        return false;
    }

private:
    struct Reply {
        int status;
        QByteArray body;
    };
    static QByteArray json(const QJsonObject &o) { return QJsonDocument(o).toJson(QJsonDocument::Compact); }

    QString activeProvider() const {
        return settings.value(QStringLiteral("llm")).toObject().value(QStringLiteral("active")).toString();
    }
    QString selectedModel() const {
        return settings.value(QStringLiteral("transcriber")).toObject().value(QStringLiteral("model_id")).toString();
    }

    // The same rules as the sidecar's setup_status_service.rs.
    QJsonObject setupStatus() const {
        const QString model = selectedModel();
        const bool downloading = installStatus == QLatin1String("downloading");
        const QString transcription = modelFileMissing ? QStringLiteral("missing")
                                      : !model.isEmpty() ? QStringLiteral("ready")
                                      : downloading ? QStringLiteral("downloading")
                                                    : QStringLiteral("missing");
        const QString provider = activeProvider();
        const QString minutes = keysLocked ? QStringLiteral("locked")
                                : provider == QLatin1String("ollama") || secrets.contains(provider)
                                    ? QStringLiteral("ready")
                                    : settings.value(QStringLiteral("minutes_setup_skipped")).toBool()
                                          ? QStringLiteral("skipped")
                                          : QStringLiteral("missing");
        return {{QStringLiteral("transcription"),
                 QJsonObject{{QStringLiteral("state"), transcription},
                             {QStringLiteral("reason"),
                              modelFileMissing ? QJsonValue(QStringLiteral("model_missing"))
                              : model.isEmpty() ? QJsonValue(QStringLiteral("model_not_selected"))
                                                : QJsonValue()},
                             {QStringLiteral("source"), QStringLiteral("managed")},
                             {QStringLiteral("model_id"), model.isEmpty() ? QJsonValue() : QJsonValue(model)},
                             {QStringLiteral("recommended_model_id"), QStringLiteral("small")},
                             {QStringLiteral("installation"), QJsonValue()}}},
                {QStringLiteral("minutes"),
                 QJsonObject{{QStringLiteral("state"), minutes}, {QStringLiteral("provider"), provider}}},
                {QStringLiteral("cpu_supported"), true},
                {QStringLiteral("recovered_recordings"), recovered}};
    }

    Reply route(const QByteArray &method, const QString &path, const QByteArray &body) {
        const QJsonObject request = QJsonDocument::fromJson(body).object();

        if (method == "GET" && path == QLatin1String("/api/v1/setup-status"))
            return {200, json(setupStatus())};
        if (method == "GET" && path == QLatin1String("/api/v1/settings"))
            return {200, json(settings)};
        if (method == "PUT" && path == QLatin1String("/api/v1/settings")) {
            settings = request;
            return {200, json(settings)};
        }
        if (method == "PUT" && path == QLatin1String("/api/v1/settings/secret")) {
            secrets[request.value(QStringLiteral("provider")).toString()] =
                request.value(QStringLiteral("value")).toString();
            return {204, {}};
        }
        if (method == "POST" && path == QLatin1String("/api/v1/settings/test")) {
            if (providerCheckFails.isEmpty())
                return {204, {}};
            return {502, json(QJsonObject{{QStringLiteral("error_class"), providerCheckFails},
                                          {QStringLiteral("message"), QStringLiteral("provider said no")}})};
        }
        if (method == "GET" && path == QLatin1String("/api/v1/transcription-models"))
            return {200, json(QJsonObject{
                             {QStringLiteral("models"),
                              QJsonArray{QJsonObject{{QStringLiteral("id"), QStringLiteral("small")},
                                                     {QStringLiteral("display_name"), QStringLiteral("Small")},
                                                     {QStringLiteral("approximate_size"),
                                                      QStringLiteral("466 MiB")}}}}})};
        if (method == "POST" && path == QLatin1String("/api/v1/transcription-models/small/install")) {
            installStatus = QStringLiteral("downloading");
            return {200, json(QJsonObject{{QStringLiteral("job_id"), QStringLiteral("job-1")}})};
        }
        if (method == "GET" && path == QLatin1String("/api/v1/transcription-models/installations/job-1"))
            return {200, json(QJsonObject{
                             {QStringLiteral("job_id"), QStringLiteral("job-1")},
                             {QStringLiteral("model_id"), QStringLiteral("small")},
                             {QStringLiteral("status"), installStatus},
                             {QStringLiteral("percent"), installPercent},
                             {QStringLiteral("error"),
                              installStatus == QLatin1String("failed")
                                  ? QJsonValue(QStringLiteral("no space left on device"))
                                  : QJsonValue()}})};
        if (method == "GET" && path == QLatin1String("/api/v1/audio/devices")) {
            QJsonArray inputs;
            if (hasMicrophone)
                inputs.append(QJsonObject{{QStringLiteral("id"), QStringLiteral("mic-1")},
                                          {QStringLiteral("label"), QStringLiteral("Built-in")},
                                          {QStringLiteral("is_default"), true}});
            return {200, json(QJsonObject{{QStringLiteral("input"), inputs},
                                          {QStringLiteral("output"), QJsonArray()},
                                          {QStringLiteral("system_selectable"), false}})};
        }
        if (method == "POST" && path == QLatin1String("/api/v1/audio/monitor"))
            return {200, json(QJsonObject{{QStringLiteral("resolved"), QJsonObject()}})};
        if (method == "GET" && path.startsWith(QLatin1String("/api/v1/audio/monitor/")))
            return {200, json(QJsonObject{{QStringLiteral("level"), micLevel},
                                          {QStringLiteral("peak_db"), -20}})};
        if (method == "DELETE" && path.startsWith(QLatin1String("/api/v1/audio/monitor/")))
            return {204, {}};

        return {404, QByteArrayLiteral(R"({"error":"not found"})")};
    }

    void onReadyRead(QTcpSocket *sock) {
        QByteArray &buffer = m_buffers[sock];
        buffer += sock->readAll();
        const int headerEnd = buffer.indexOf("\r\n\r\n");
        if (headerEnd < 0)
            return;
        static const QRegularExpression lengthHeader(
            QStringLiteral("content-length:\\s*(\\d+)"), QRegularExpression::CaseInsensitiveOption);
        const QRegularExpressionMatch length = lengthHeader.match(QString::fromLatin1(buffer.left(headerEnd)));
        const int bodySize = length.hasMatch() ? length.captured(1).toInt() : 0;
        if (buffer.size() < headerEnd + 4 + bodySize)
            return;

        const QList<QByteArray> requestLine = buffer.left(buffer.indexOf("\r\n")).split(' ');
        const QString path = QString::fromLatin1(requestLine.value(1));
        requests << QString::fromLatin1(requestLine.value(0)) + QLatin1Char(' ') + path;
        const Reply reply = route(requestLine.value(0), path, buffer.mid(headerEnd + 4, bodySize));
        m_buffers.remove(sock);
        sock->write("HTTP/1.1 " + QByteArray::number(reply.status) + " X\r\n"
                    "Content-Type: application/json\r\n"
                    "Content-Length: " + QByteArray::number(reply.body.size()) + "\r\n"
                    "Connection: close\r\n\r\n" + reply.body);
        sock->disconnectFromHost();
    }

    QTcpServer m_server;
    QHash<QTcpSocket *, QByteArray> m_buffers;
};

// Stands in for main.cpp's MicrophonePermission, which would ask macOS.
class FakeMicrophonePermission final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)

public:
    QString status() const { return m_status; }
    void setStatus(const QString &status) {
        m_status = status;
        emit statusChanged();
    }
    // What the user answers when asked.
    QString answer = QStringLiteral("granted");
    int requests = 0;

    Q_INVOKABLE void request() {
        ++requests;
        setStatus(answer);
    }
    Q_INVOKABLE void refresh() { emit statusChanged(); }

signals:
    void statusChanged();

private:
    QString m_status = QStringLiteral("undetermined");
};

class FakePreferences final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool microphoneTested READ microphoneTested NOTIFY microphoneTestedChanged)

public:
    bool microphoneTested() const { return m_tested; }
    Q_INVOKABLE void markMicrophoneTested() {
        m_tested = true;
        emit microphoneTestedChanged();
    }

signals:
    void microphoneTestedChanged();

private:
    bool m_tested = false;
};

// The steps in a window, talking to a fake sidecar. Members are ordered so the
// QML goes first and what it talks to goes last.
struct App {
    FakeSidecar sidecar;
    FakeMicrophonePermission permission;
    FakePreferences preferences;
    ApiClient api;
    QQmlEngine engine;
    std::unique_ptr<QObject> window;
    QQuickItem *steps = nullptr;

    // `notices`: load the launch notices instead of the setup steps.
    bool start(bool notices = false) {
        if (!sidecar.listen())
            return false;
        api.configure(sidecar.baseUrl(), QStringLiteral("test-token"));

        engine.addImportPath(QStringLiteral(MA_FIRST_RUN_STEPS_QML_IMPORT_DIR));
        QQmlContext *ctx = engine.rootContext();
        ctx->setContextProperty(QStringLiteral("api"), &api);
        ctx->setContextProperty(QStringLiteral("microphonePermission"), &permission);
        ctx->setContextProperty(QStringLiteral("testPreferences"), &preferences);

        QQmlComponent component(&engine);
        if (notices)
            component.setData(R"qml(
            import QtQuick
            import QtQuick.Controls
            import MeetingAssistant

            ApplicationWindow {
                property alias steps: noticesItem
                width: 640
                height: 400
                visible: true
                LaunchNotices {
                    id: noticesItem
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.margins: 24
                    preferences: testPreferences
                    meetings: [{ "id": "m-1", "name": "Планёрка", "has_transcript": false }]
                }
            }
        )qml", QUrl());
        else
            component.setData(R"qml(
            import QtQuick
            import QtQuick.Controls
            import MeetingAssistant

            ApplicationWindow {
                property alias steps: stepsItem
                width: 640
                height: 900
                visible: true
                FirstRunSteps {
                    id: stepsItem
                    anchors.fill: parent
                    anchors.margins: 24
                    preferences: testPreferences
                }
            }
        )qml", QUrl());
        window.reset(component.create());
        if (!window) {
            qWarning().noquote() << component.errorString();
            return false;
        }
        steps = window->property("steps").value<QQuickItem *>();
        // AppShell asks once per launch; the notices only read the answer.
        if (notices)
            QMetaObject::invokeMethod(setup(), "refresh");
        // Every scenario starts from a pane that knows the setup status.
        return steps && QTest::qWaitFor([&] { return setup()->property("loaded").toBool(); }, 5000);
    }

    QObject *setup() { return engine.singletonInstance<QObject *>("MeetingAssistant", "SetupStore"); }
    QStringList noticeIds() const {
        QStringList ids;
        for (const QVariant &n : steps->property("items").toList())
            ids << n.toMap().value(QStringLiteral("id")).toString();
        return ids;
    }
    // Repeater delegates are visual children only, which findChild() misses.
    static QQuickItem *byName(QQuickItem *root, const QString &objectName) {
        if (root->objectName() == objectName)
            return root;
        for (QQuickItem *child : root->childItems())
            if (QQuickItem *hit = byName(child, objectName))
                return hit;
        return nullptr;
    }
    // A button inside the notice with this id.
    QQuickItem *noticeButton(const QString &id, const QString &button) const {
        QQuickItem *card = byName(steps, QStringLiteral("notice-") + id);
        return card ? byName(card, button) : nullptr;
    }
    void pressNotice(const QString &id, const QString &button) const {
        QQuickItem *target = noticeButton(id, button);
        QVERIFY2(target, qPrintable(id + QLatin1Char('/') + button));
        QMetaObject::invokeMethod(target, "clicked");
    }
    QQuickItem *item(const QString &objectName) const {
        return steps->findChild<QQuickItem *>(objectName);
    }
    QString stepStatus(const QString &step) const { return item(step)->property("status").toString(); }
    QString stepSummary(const QString &step) const { return item(step)->property("summary").toString(); }
    bool enabled(const QString &button) const { return item(button)->property("enabled").toBool(); }
    // A press of the button, as the user's click delivers it.
    void press(const QString &button) const {
        QVERIFY2(item(button)->isVisible() && enabled(button), qPrintable(button));
        QMetaObject::invokeMethod(item(button), "clicked");
    }
};

} // namespace

class FirstRunStepsTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void nothingIsAskedOfTheSystemUntilTheUserActs();
    void newProfileShowsThreeOpenStepsAndHoldsTheStartButton();
    void microphoneTestAsksOnceThenHearsTheUser();
    void deniedMicrophoneIsExplainedAndNeverOpened();
    void missingMicrophoneIsSaidAsSuch();
    void modelDownloadShowsProgressThenSelectsTheModel();
    void failedModelDownloadSaysWhyAndCanBeRetried();
    void typedKeyIsSavedCheckedAndFinishesTheStep();
    void rejectedKeyKeepsTheStepOpenWithTheReason();
    void laterForMinutesFinishesTheStepWithoutAKey();
    void ollamaNeedsNoKey();
    void skipAndStartAreReportedToTheScreen();
    void launchIsQuietWhenNothingBroke();
    void recoveredRecordingIsAnnouncedAndOpens();
    void brokenSetupIsAnnouncedWithItsFix();
    void refusedMicrophoneIsAnnouncedOnlyIfItWorkedBefore();
    void closedNoticeStaysClosed();
};

void FirstRunStepsTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_FIRST_RUN_STEPS_QML_IMPORT_DIR));
}

// The owner's rule for macOS: a system dialog appears only as the result of
// something the user just pressed. Showing the pane must therefore not ask for
// the microphone, open a capture device or read a key.
void FirstRunStepsTest::nothingIsAskedOfTheSystemUntilTheUserActs() {
    App app;
    QVERIFY(app.start());
    QTest::qWait(300);

    QCOMPARE(app.permission.requests, 0);
    QVERIFY(!app.sidecar.sawPrefix(QStringLiteral("POST /api/v1/audio/monitor")));
    QVERIFY(!app.sidecar.sawPrefix(QStringLiteral("GET /api/v1/audio/devices")));
    QVERIFY(!app.sidecar.sawPrefix(QStringLiteral("POST /api/v1/settings/test")));
    QVERIFY(!app.sidecar.sawPrefix(QStringLiteral("POST /api/v1/transcription-models")));
}

void FirstRunStepsTest::newProfileShowsThreeOpenStepsAndHoldsTheStartButton() {
    App app;
    QVERIFY(app.start());

    QCOMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("todo"));
    QCOMPARE(app.stepStatus(QStringLiteral("modelStep")), QStringLiteral("todo"));
    QCOMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("todo"));
    // The recommended model is named with its size once the catalogue is in.
    QTRY_VERIFY(app.stepSummary(QStringLiteral("modelStep")).contains(QStringLiteral("Small, 466 MiB")));
    // Only the microphone gates recording.
    QVERIFY(!app.enabled(QStringLiteral("startButton")));
    QCOMPARE(app.item(QStringLiteral("startCaption"))->property("text").toString(),
             QStringLiteral("Сначала проверьте микрофон."));
}

void FirstRunStepsTest::microphoneTestAsksOnceThenHearsTheUser() {
    App app;
    QVERIFY(app.start());

    app.press(QStringLiteral("micTestButton"));

    QCOMPARE(app.permission.requests, 1);
    QTRY_COMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/audio/monitor")), 1);
    QCOMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("busy"));
    QVERIFY(!app.enabled(QStringLiteral("startButton")));

    // The user speaks.
    app.sidecar.micLevel = 0.4;
    QTRY_VERIFY(app.steps->property("micHeard").toBool());
    QVERIFY(app.preferences.microphoneTested());
    QVERIFY(app.enabled(QStringLiteral("startButton")));

    // The microphone is let go shortly after, and the step folds to done.
    QTRY_VERIFY_WITH_TIMEOUT(app.sidecar.sawPrefix(QStringLiteral("DELETE /api/v1/audio/monitor/")), 6000);
    QTRY_COMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("done"));
}

void FirstRunStepsTest::deniedMicrophoneIsExplainedAndNeverOpened() {
    App app;
    app.permission.answer = QStringLiteral("denied");
    QVERIFY(app.start());

    app.press(QStringLiteral("micTestButton"));

    QTRY_COMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("attention"));
    QVERIFY(app.stepSummary(QStringLiteral("micStep")).contains(QStringLiteral("Доступ к микрофону запрещён")));
    QTest::qWait(200);
    QVERIFY(!app.sidecar.sawPrefix(QStringLiteral("POST /api/v1/audio/monitor")));
    QVERIFY(!app.enabled(QStringLiteral("startButton")));

    // The user allows it in the system settings and tries again: no second
    // system prompt, straight to listening.
    app.permission.setStatus(QStringLiteral("granted"));
    app.press(QStringLiteral("micTestButton"));
    QTRY_COMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/audio/monitor")), 1);
    QCOMPARE(app.permission.requests, 1);
}

void FirstRunStepsTest::missingMicrophoneIsSaidAsSuch() {
    App app;
    app.sidecar.hasMicrophone = false;
    QVERIFY(app.start());

    app.press(QStringLiteral("micTestButton"));

    QTRY_VERIFY(app.stepSummary(QStringLiteral("micStep")).contains(QStringLiteral("Микрофон не найден")));
    QCOMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("attention"));
    // Nothing is left listening, and recording stays held back.
    QTRY_VERIFY(app.steps->property("monitorId").toString().isEmpty());
    QVERIFY(!app.enabled(QStringLiteral("startButton")));
}

void FirstRunStepsTest::modelDownloadShowsProgressThenSelectsTheModel() {
    App app;
    QVERIFY(app.start());
    QTRY_VERIFY(app.enabled(QStringLiteral("modelDownloadButton")));

    app.sidecar.installPercent = 37.5;
    app.press(QStringLiteral("modelDownloadButton"));

    QTRY_COMPARE(app.stepStatus(QStringLiteral("modelStep")), QStringLiteral("busy"));
    QTRY_VERIFY(app.stepSummary(QStringLiteral("modelStep")).contains(QStringLiteral("37%")));

    app.sidecar.installStatus = QStringLiteral("done");

    // A downloaded model nobody selected would still fail the first
    // transcription, so the step selects it.
    QTRY_COMPARE(app.stepStatus(QStringLiteral("modelStep")), QStringLiteral("done"));
    QCOMPARE(app.sidecar.settings.value(QStringLiteral("transcriber")).toObject()
                 .value(QStringLiteral("model_id")).toString(),
             QStringLiteral("small"));
    QCOMPARE(app.sidecar.count(QStringLiteral("PUT /api/v1/settings")), 1);
}

void FirstRunStepsTest::failedModelDownloadSaysWhyAndCanBeRetried() {
    App app;
    QVERIFY(app.start());
    QTRY_VERIFY(app.enabled(QStringLiteral("modelDownloadButton")));

    app.press(QStringLiteral("modelDownloadButton"));
    QTRY_COMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/transcription-models/small/install")), 1);
    QCOMPARE(app.stepStatus(QStringLiteral("modelStep")), QStringLiteral("busy"));
    app.sidecar.installStatus = QStringLiteral("failed");

    QTRY_COMPARE(app.stepStatus(QStringLiteral("modelStep")), QStringLiteral("attention"));
    QVERIFY(app.stepSummary(QStringLiteral("modelStep")).contains(QStringLiteral("no space left on device")));
    QCOMPARE(app.item(QStringLiteral("modelDownloadButton"))->property("text").toString(),
             QStringLiteral("Повторить"));

    app.press(QStringLiteral("modelDownloadButton"));
    QTRY_COMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/transcription-models/small/install")), 2);
}

void FirstRunStepsTest::typedKeyIsSavedCheckedAndFinishesTheStep() {
    App app;
    QVERIFY(app.start());

    app.item(QStringLiteral("keyField"))->setProperty("text", QStringLiteral("  sk-test  "));
    app.press(QStringLiteral("minutesCheckButton"));

    QTRY_COMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("done"));
    QCOMPARE(app.sidecar.secrets.value(QStringLiteral("anthropic")), QStringLiteral("sk-test"));
    QCOMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/settings/test")), 1);
    QVERIFY(app.stepSummary(QStringLiteral("minutesStep")).contains(QStringLiteral("Anthropic")));
    // The key does not linger in the field.
    QCOMPARE(app.item(QStringLiteral("keyField"))->property("text").toString(), QString());
}

void FirstRunStepsTest::rejectedKeyKeepsTheStepOpenWithTheReason() {
    App app;
    app.sidecar.providerCheckFails = QStringLiteral("api_auth");
    QVERIFY(app.start());

    // No key at all: nothing is sent.
    app.press(QStringLiteral("minutesCheckButton"));
    QCOMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("attention"));
    QCOMPARE(app.stepSummary(QStringLiteral("minutesStep")), QStringLiteral("Введите API-ключ."));
    QTest::qWait(150);
    QCOMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/settings/test")), 0);

    // A key the provider rejects: stored, so the status says "ready", but the
    // step must not read as done.
    app.item(QStringLiteral("keyField"))->setProperty("text", QStringLiteral("sk-wrong"));
    app.press(QStringLiteral("minutesCheckButton"));
    QTRY_COMPARE(app.sidecar.count(QStringLiteral("POST /api/v1/settings/test")), 1);
    QTRY_COMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("attention"));
    QVERIFY(app.stepSummary(QStringLiteral("minutesStep")).contains(QStringLiteral("Провайдер не принял ключ")));
}

void FirstRunStepsTest::laterForMinutesFinishesTheStepWithoutAKey() {
    App app;
    QVERIFY(app.start());

    app.press(QStringLiteral("minutesLaterButton"));

    QTRY_COMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("done"));
    QVERIFY(app.sidecar.settings.value(QStringLiteral("minutes_setup_skipped")).toBool());
    QVERIFY(app.sidecar.secrets.isEmpty());
    QVERIFY(app.stepSummary(QStringLiteral("minutesStep")).contains(QStringLiteral("Настроите позже")));
}

void FirstRunStepsTest::ollamaNeedsNoKey() {
    App app;
    QVERIFY(app.start());

    app.steps->setProperty("provider", QStringLiteral("ollama"));
    QVERIFY(!app.item(QStringLiteral("keyField"))->isVisible());
    app.press(QStringLiteral("minutesCheckButton"));

    QTRY_COMPARE(app.stepStatus(QStringLiteral("minutesStep")), QStringLiteral("done"));
    QCOMPARE(app.sidecar.settings.value(QStringLiteral("llm")).toObject()
                 .value(QStringLiteral("active")).toString(),
             QStringLiteral("ollama"));
    QVERIFY(app.sidecar.secrets.isEmpty());
    QVERIFY(app.stepSummary(QStringLiteral("minutesStep")).contains(QStringLiteral("Ollama")));
}

void FirstRunStepsTest::skipAndStartAreReportedToTheScreen() {
    App app;
    QVERIFY(app.start());
    QSignalSpy skipped(app.steps, SIGNAL(skipped()));
    QSignalSpy started(app.steps, SIGNAL(startRecording()));

    app.press(QStringLiteral("skipButton"));
    QCOMPARE(skipped.count(), 1);

    // A microphone tested on an earlier launch keeps the step done.
    app.permission.setStatus(QStringLiteral("granted"));
    app.preferences.markMicrophoneTested();
    QTRY_COMPARE(app.stepStatus(QStringLiteral("micStep")), QStringLiteral("done"));
    app.press(QStringLiteral("startButton"));
    QCOMPARE(started.count(), 1);
}

// ── later launches ───────────────────────────────────────────────────────────

// What a user never set up or put off is not a breakage: the home screen says
// nothing about it. The recording screen is where that reminder lives.
void FirstRunStepsTest::launchIsQuietWhenNothingBroke() {
    App app;
    QVERIFY(app.start(true));
    QTest::qWait(200);

    QVERIFY(app.noticeIds().isEmpty());
    QVERIFY(!app.steps->isVisible());
}

void FirstRunStepsTest::recoveredRecordingIsAnnouncedAndOpens() {
    App app;
    app.sidecar.recovered = QJsonArray{QJsonObject{{QStringLiteral("meeting_id"), QStringLiteral("m-1")},
                                                   {QStringLiteral("name"), QStringLiteral("Планёрка")}}};
    QVERIFY(app.start(true));
    QSignalSpy opened(app.steps, SIGNAL(openMeeting(QVariant)));

    QTRY_COMPARE(app.noticeIds(), QStringList{QStringLiteral("recovered-m-1")});
    QVERIFY(app.steps->isVisible());
    QVERIFY(app.steps->property("items").toList().first().toMap()
                .value(QStringLiteral("text")).toString().contains(QStringLiteral("«Планёрка»")));

    app.pressNotice(QStringLiteral("recovered-m-1"), QStringLiteral("noticeAction"));

    QCOMPARE(opened.count(), 1);
    QCOMPARE(opened.first().first().toMap().value(QStringLiteral("id")).toString(), QStringLiteral("m-1"));
    // Opened once is told once.
    QTRY_VERIFY(app.noticeIds().isEmpty());
}

void FirstRunStepsTest::brokenSetupIsAnnouncedWithItsFix() {
    App app;
    app.sidecar.modelFileMissing = true;
    app.sidecar.keysLocked = true;
    QVERIFY(app.start(true));
    QSignalSpy settings(app.steps, SIGNAL(openSettings(int)));

    QTRY_COMPARE(app.noticeIds(), (QStringList{QStringLiteral("model-missing"), QStringLiteral("keys-locked")}));

    app.pressNotice(QStringLiteral("model-missing"), QStringLiteral("noticeAction"));
    app.pressNotice(QStringLiteral("keys-locked"), QStringLiteral("noticeAction"));
    QCOMPARE(settings.count(), 2);
    QCOMPARE(settings.at(0).first().toInt(), 0);
    QCOMPARE(settings.at(1).first().toInt(), 1);
}

void FirstRunStepsTest::refusedMicrophoneIsAnnouncedOnlyIfItWorkedBefore() {
    App app;
    app.permission.setStatus(QStringLiteral("denied"));
    QVERIFY(app.start(true));
    QTest::qWait(200);

    // Never tested: the first-run step is where that gets explained.
    QVERIFY(app.noticeIds().isEmpty());

    app.preferences.markMicrophoneTested();
    QTRY_COMPARE(app.noticeIds(), QStringList{QStringLiteral("mic-denied")});
    // Reading the status asked the user nothing.
    QCOMPARE(app.permission.requests, 0);

    app.permission.setStatus(QStringLiteral("granted"));
    QTRY_VERIFY(app.noticeIds().isEmpty());
}

void FirstRunStepsTest::closedNoticeStaysClosed() {
    App app;
    app.sidecar.modelFileMissing = true;
    QVERIFY(app.start(true));
    QTRY_COMPARE(app.noticeIds(), QStringList{QStringLiteral("model-missing")});

    app.pressNotice(QStringLiteral("model-missing"), QStringLiteral("noticeClose"));
    QTRY_VERIFY(app.noticeIds().isEmpty());

    // A later refresh in the same run does not bring it back.
    QMetaObject::invokeMethod(app.setup(), "refresh");
    QTest::qWait(300);
    QVERIFY(app.noticeIds().isEmpty());
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQuickStyle::setStyle(QStringLiteral("Fusion")); // as main.cpp does
    FirstRunStepsTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_first_run_steps.moc"
