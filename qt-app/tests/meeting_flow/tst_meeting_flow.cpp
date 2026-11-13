// Screen flows around a job: generate → minutes, import → transcript.
//
// These bugs lived in the wiring between screens, stores and layouts, so this
// test runs the real AppShell, the real screens and the real stores, plus the
// real ApiClient and JobPoller. Only three things are not real:
//   - the sidecar: FakeSidecar answers the same HTTP routes, because a real job
//     needs a Whisper model and an LLM, which CI has neither of;
//   - AudioPlayer: its media backend adds nothing here, and the stub keeps its
//     place and height in the page layout;
//   - the screens no flow here looks into (see stubs/).
//
// No translator is loaded, so visible text is the Russian source text.

#include "ApiClient.h"
#include "EmbeddedQmldirGuard.h"
#include "JobPoller.h"
#include "PathUtils.h"

#include <QGuiApplication>
#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMap>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QRegularExpression>
#include <QTcpServer>
#include <QTcpSocket>
#include <QtTest>

#include <functional>
#include <memory>

namespace {

constexpr qint64 kCreatedAt = 1793982658;

// A finished job's card stays for ActiveJobsStore.terminalTtlMs (4 s), and the
// poller falls back to polling once a second, so waits for a sweep need room.
constexpr int kSweepTimeoutMs = 10000;

const QString kNoMinutesTitle = QStringLiteral("Протокол ещё не сгенерирован");
const QString kGenerateMinutes = QStringLiteral("Сгенерировать протокол");

// The sidecar's job and meeting routes, driven by the test instead of a worker.
class FakeSidecar final : public QObject {
    Q_OBJECT

public:
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

    // Answer a recording start as macOS does without Screen Recording access.
    bool deniesScreenCapture = false;

    // Returns the meeting as the list route reports it, ready for shell.show*().
    QVariantMap addMeeting(const QString &id, bool hasTranscript) {
        m_meetings[id] = Meeting{id, hasTranscript, QString()};
        m_meetingOrder << id;
        return summary(id).toVariantMap();
    }

    QString liveJobOf(const QString &meetingId) const {
        for (const Job &job : m_jobs)
            if (job.meetingId == meetingId && !isTerminal(job))
                return job.id;
        return {};
    }

    // What the worker does when a job succeeds: store the result, then go done.
    void finishJob(const QString &jobId) {
        Job &job = m_jobs[jobId];
        Meeting &meeting = m_meetings[job.meetingId];
        if (job.kind == QLatin1String("regenerate_protocol"))
            meeting.protocol = QStringLiteral(
                "# Итоги\n\nКоманда обсудила план выпуска и договорилась о сроках.");
        else
            meeting.hasTranscript = true;
        job.status = QStringLiteral("done");
    }
    void failJob(const QString &jobId) { m_jobs[jobId].status = QStringLiteral("failed"); }

    QStringList reprocessKinds() const { return m_reprocessKinds; }
    // The template_name of each reprocess request, "" when it named none.
    QStringList reprocessTemplates() const { return m_reprocessTemplates; }
    int hits(const QString &path) const { return m_hits.value(path); }

private:
    struct Meeting {
        QString id;
        bool hasTranscript = false;
        QString protocol;
    };
    struct Job {
        QString id;
        QString meetingId;
        QString kind;
        QString status;
    };
    struct Reply {
        int status;
        QByteArray body;
    };

    static bool isTerminal(const Job &job) {
        return job.status == QLatin1String("done") || job.status == QLatin1String("failed");
    }
    static QByteArray json(const QJsonObject &o) { return QJsonDocument(o).toJson(QJsonDocument::Compact); }
    static QByteArray json(const QJsonArray &a) { return QJsonDocument(a).toJson(QJsonDocument::Compact); }

    QJsonObject summary(const QString &id) const {
        const Meeting &m = m_meetings[id];
        return {{QStringLiteral("id"), m.id},
                {QStringLiteral("name"), m.id},
                {QStringLiteral("audio_path"), QStringLiteral("/sandbox/%1.wav").arg(m.id)},
                {QStringLiteral("has_transcript"), m.hasTranscript},
                {QStringLiteral("created_at"), kCreatedAt}};
    }
    QJsonObject jobJson(const Job &job) const {
        const bool failed = job.status == QLatin1String("failed");
        QJsonObject o{{QStringLiteral("id"), job.id},
                      {QStringLiteral("meeting_id"), job.meetingId},
                      {QStringLiteral("kind"), job.kind},
                      {QStringLiteral("status"), job.status},
                      {QStringLiteral("attempts"), 1},
                      {QStringLiteral("created_at"), 1},
                      {QStringLiteral("updated_at"), 2},
                      {QStringLiteral("last_error"),
                       failed ? QJsonValue(QStringLiteral("decoder failed")) : QJsonValue()},
                      {QStringLiteral("error_class"),
                       failed ? QJsonValue(QStringLiteral("unknown")) : QJsonValue()}};
        if (!isTerminal(job))
            o[QStringLiteral("progress")] =
                QJsonObject{{QStringLiteral("stage"), QStringLiteral("transcribing")},
                            {QStringLiteral("sub"), QString()},
                            {QStringLiteral("percent"), 40}};
        else
            o[QStringLiteral("progress")] = QJsonValue();
        return o;
    }
    QString enqueue(const QString &meetingId, const QString &kind) {
        const QString id = QStringLiteral("job-%1").arg(m_jobs.size() + 1);
        m_jobs[id] = Job{id, meetingId, kind, QStringLiteral("running")};
        return id;
    }

    Reply route(const QByteArray &method, const QString &path, const QByteArray &body) {
        static const QRegularExpression meetingRoute(QStringLiteral("^/api/v1/meetings/([^/]+)$"));
        static const QRegularExpression reprocessRoute(
            QStringLiteral("^/api/v1/meetings/([^/]+)/reprocess$"));
        static const QRegularExpression jobRoute(QStringLiteral("^/api/v1/jobs/([^/]+)$"));

        if (method == "GET" && path == QLatin1String("/api/v1/meetings")) {
            QJsonArray list;
            for (const QString &id : m_meetingOrder)
                list.append(summary(id));
            return {200, json(list)};
        }
        if (method == "POST" && path == QLatin1String("/api/v1/meetings/import")) {
            const QJsonObject req = QJsonDocument::fromJson(body).object();
            const QString file = req.value(QStringLiteral("path")).toString();
            const QString id = file.section(QLatin1Char('/'), -1).section(QLatin1Char('.'), 0, 0);
            addMeeting(id, false);
            QJsonObject reply = summary(id);
            reply.remove(QStringLiteral("has_transcript"));
            reply.remove(QStringLiteral("created_at"));
            reply[QStringLiteral("job_id")] = enqueue(id, QStringLiteral("transcribe"));
            return {201, json(reply)};
        }
        if (method == "GET" && path == QLatin1String("/api/v1/active-jobs")) {
            QJsonArray list;
            for (const Job &job : m_jobs)
                if (!isTerminal(job))
                    list.append(jobJson(job));
            return {200, json(list)};
        }
        if (method == "GET" && path == QLatin1String("/api/v1/settings"))
            return {200, json(QJsonObject{{QStringLiteral("llm"),
                                           QJsonObject{{QStringLiteral("active"), QStringLiteral("ollama")}}},
                                          {QStringLiteral("recording"),
                                           QJsonObject{{QStringLiteral("source"), QStringLiteral("mixed")},
                                                       {QStringLiteral("echo_cancel"), false}}}})};
        if (method == "POST" && path == QLatin1String("/api/v1/recordings") && deniesScreenCapture)
            return {500, json(QJsonObject{
                             {QStringLiteral("error"),
                              QStringLiteral("recording error: ScreenCaptureKit error: No shareable content "
                                             "available. Grant Screen Recording in "
                                             "x-apple.systempreferences:com.apple.preference.security"
                                             "?Privacy_ScreenCapture")}})};
        if (method == "POST" && path == QLatin1String("/api/v1/recordings")) {
            // What the macOS sidecar answers for a mixed recording: the mic by
            // name, and the system mix with an empty label for the GUI to name.
            const QJsonObject resolved{{QStringLiteral("mic"), QStringLiteral("MacBook Pro Microphone")},
                                       {QStringLiteral("system"), QString()}};
            return {201, json(QJsonObject{{QStringLiteral("id"), QStringLiteral("rec-1")},
                                          {QStringLiteral("name"), QStringLiteral("Планёрка")},
                                          {QStringLiteral("audio_path"), QStringLiteral("/sandbox/rec-1.wav")},
                                          {QStringLiteral("created_at"), kCreatedAt},
                                          {QStringLiteral("resolved"), resolved}})};
        }
        if (method == "GET" && path == QLatin1String("/api/v1/audio/devices"))
            return {200, json(QJsonObject{{QStringLiteral("input"), QJsonArray()},
                                          {QStringLiteral("output"), QJsonArray()},
                                          {QStringLiteral("system_selectable"), false}})};

        QRegularExpressionMatch match = reprocessRoute.match(path);
        if (method == "POST" && match.hasMatch()) {
            const QJsonObject request = QJsonDocument::fromJson(body).object();
            const QString kind = request.value(QStringLiteral("kind")).toString();
            m_reprocessKinds << kind;
            m_reprocessTemplates << request.value(QStringLiteral("template_name")).toString();
            const QString backendKind = kind == QLatin1String("protocol")
                                            ? QStringLiteral("regenerate_protocol")
                                            : QStringLiteral("reprocess_transcribe");
            return {202, json(QJsonObject{{QStringLiteral("job_id"), enqueue(match.captured(1), backendKind)}})};
        }
        match = meetingRoute.match(path);
        if (method == "GET" && match.hasMatch() && m_meetings.contains(match.captured(1))) {
            const Meeting &m = m_meetings[match.captured(1)];
            QJsonObject detail = summary(m.id);
            detail[QStringLiteral("protocol")] = m.protocol.isEmpty() ? QJsonValue() : QJsonValue(m.protocol);
            detail[QStringLiteral("structured_protocol")] = QJsonValue();
            detail[QStringLiteral("protocol_usage")] = QJsonValue();
            return {200, json(detail)};
        }
        match = jobRoute.match(path);
        if (method == "GET" && match.hasMatch() && m_jobs.contains(match.captured(1)))
            return {200, json(jobJson(m_jobs[match.captured(1)]))};

        // Includes /jobs/:id/events: a refused stream sends JobPoller to polling.
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
        ++m_hits[path];
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
    QHash<QString, Meeting> m_meetings;
    QStringList m_meetingOrder;
    QMap<QString, Job> m_jobs;
    QStringList m_reprocessKinds;
    QStringList m_reprocessTemplates;
    QHash<QString, int> m_hits;
};

QQuickItem *findVisible(QQuickItem *root, const std::function<bool(QQuickItem *)> &matches) {
    if (!root->isVisible())
        return nullptr;
    if (matches(root))
        return root;
    const QList<QQuickItem *> children = root->childItems();
    for (QQuickItem *child : children)
        if (QQuickItem *hit = findVisible(child, matches))
            return hit;
    return nullptr;
}

QQuickItem *findStack(QQuickItem *root) {
    if (root->inherits("QQuickStackView"))
        return root;
    const QList<QQuickItem *> children = root->childItems();
    for (QQuickItem *child : children)
        if (QQuickItem *hit = findStack(child))
            return hit;
    return nullptr;
}

// One running app: the real shell in a window, talking to a fake sidecar. Members
// are ordered so the QML goes first and the sidecar it talks to goes last.
// Stands in for main.cpp's ScreenCapturePermission, which would run tccutil.
class FakeScreenCapturePermission final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool canReset READ canReset CONSTANT)

public:
    bool canReset() const { return true; }
    Q_INVOKABLE bool reset() {
        ++resets;
        return true;
    }
    int resets = 0;
};

struct App {
    FakeSidecar sidecar;
    FakeScreenCapturePermission permission;
    ApiClient api;
    PathUtils pathUtils;
    QQmlEngine engine;
    std::unique_ptr<QObject> window;
    QQuickItem *shell = nullptr;
    QQuickItem *stack = nullptr;

    bool start() {
        if (!sidecar.listen())
            return false;
        api.configure(sidecar.baseUrl(), QStringLiteral("test-token"));

        engine.addImportPath(QStringLiteral(MA_MEETING_FLOW_QML_IMPORT_DIR));
        QQmlContext *ctx = engine.rootContext();
        ctx->setContextProperty(QStringLiteral("api"), &api);
        ctx->setContextProperty(QStringLiteral("pathUtils"), &pathUtils);
        ctx->setContextProperty(QStringLiteral("screenCapturePermission"), &permission);
        ctx->setContextProperty(QStringLiteral("appPreferences"), QVariant::fromValue<QObject *>(nullptr));

        QQmlComponent component(&engine);
        component.setData(R"qml(
            import QtQuick
            import QtQuick.Controls
            import MeetingAssistant

            ApplicationWindow {
                property alias shell: shellItem
                width: 1120
                height: 820
                visible: true
                AppShell { id: shellItem; anchors.fill: parent }
            }
        )qml", QUrl());
        window.reset(component.create());
        if (!window) {
            qWarning().noquote() << component.errorString();
            return false;
        }
        shell = window->property("shell").value<QQuickItem *>();
        stack = findStack(shell);
        return stack != nullptr;
    }

    QQuickWindow *quickWindow() const { return qobject_cast<QQuickWindow *>(window.get()); }
    QObject *jobs() { return engine.singletonInstance<QObject *>("MeetingAssistant", "ActiveJobsStore"); }
    QObject *store() const { return shell->property("storeRef").value<QObject *>(); }
    QQuickItem *page() const { return stack->property("currentItem").value<QQuickItem *>(); }

    // QML types get a generated class name ("MeetingDetailScreen_QMLTYPE_42").
    QString pageName() const {
        QQuickItem *current = page();
        return current ? QString::fromLatin1(current->metaObject()->className()).section(QLatin1Char('_'), 0, 0)
                       : QString();
    }

    bool isTracked(const QString &meetingId) {
        QVariant active;
        QMetaObject::invokeMethod(jobs(), "isActive", Q_RETURN_ARG(QVariant, active),
                                  Q_ARG(QVariant, meetingId));
        return active.toBool();
    }

    // What the sidebar was last told about a meeting.
    bool listedWithTranscript(const QString &meetingId) const {
        const QVariantList meetings = store()->property("meetings").toList();
        for (const QVariant &meeting : meetings) {
            const QVariantMap m = meeting.toMap();
            if (m.value(QStringLiteral("id")).toString() == meetingId)
                return m.value(QStringLiteral("has_transcript")).toBool();
        }
        return false;
    }

    QQuickItem *visibleText(const QString &text) const {
        return findVisible(quickWindow()->contentItem(), [&text](QQuickItem *item) {
            return item->property("text").toString() == text;
        });
    }

    QQuickItem *visibleTextContaining(const QString &part) const {
        return findVisible(quickWindow()->contentItem(), [&part](QQuickItem *item) {
            return item->property("text").toString().contains(part);
        });
    }

    // Clicks until `done` holds: a page pushed just now may not be laid out yet.
    bool clickUntil(const QString &buttonText, const std::function<bool()> &done) const {
        return QTest::qWaitFor([&] {
            QQuickItem *button = findVisible(quickWindow()->contentItem(), [&buttonText](QQuickItem *item) {
                return item->property("text").toString() == buttonText
                    && item->metaObject()->indexOfSignal("clicked()") >= 0;
            });
            if (button)
                QTest::mouseClick(quickWindow(), Qt::LeftButton, Qt::NoModifier,
                                  button->mapToScene(QPointF(button->width() / 2, button->height() / 2)).toPoint());
            return done();
        }, 5000);
    }

    // Distance from the audio player down to the document on the detail screen.
    qreal gapAboveDocument() const {
        QQuickItem *current = page();
        QQuickItem *player = findVisible(current, [](QQuickItem *item) {
            return QString::fromLatin1(item->metaObject()->className()).startsWith(QStringLiteral("AudioPlayer"));
        });
        QQuickItem *document = findVisible(current, [](QQuickItem *item) { return item->inherits("QQuickScrollView"); });
        if (!player || !document)
            return -1;
        return document->mapToScene(QPointF(0, 0)).y() - player->mapToScene(QPointF(0, player->height())).y();
    }

    // Generate from a meeting that already has its transcript: one job, then the
    // minutes screen with that job's finished card still up.
    bool generateMinutes(const QVariantMap &meeting) {
        const QString id = meeting.value(QStringLiteral("id")).toString();
        QMetaObject::invokeMethod(shell, "showGenerate", Q_ARG(QVariant, meeting), Q_ARG(QVariant, true));
        if (!QTest::qWaitFor([&] { return !sidecar.liveJobOf(id).isEmpty(); }, 5000))
            return false;
        sidecar.finishJob(sidecar.liveJobOf(id));
        return QTest::qWaitFor(
            [&] { return pageName() == QLatin1String("MeetingDetailScreen") && page()->property("hasProtocol").toBool(); },
            5000);
    }

    void importFile(const QString &path) {
        QMetaObject::invokeMethod(shell, "showNewRecording");
        QMetaObject::invokeMethod(page(), "importPath", Q_ARG(QVariant, path), Q_ARG(QVariant, true));
    }
};

} // namespace

class MeetingFlowTest : public QObject {
    Q_OBJECT

private slots:
    void initTestCase();
    void minutesKeepTheMeetingDateAfterGeneration();
    void finishedCardLeavesNoGapAboveTheMinutes();
    void importShowsProgressThenTheSavedTranscript();
    void failedImportOffersToStartAgain();
    void everyFileOfABatchImportIsTracked();
    void finishedJobUpdatesTheSidebarFromAnyScreen();
    void recordingNamesTheSystemMixInTheInterfaceLanguage();
    void typedTemplateIsUsedWithoutPressingEnter();
    void permissionDialogResetsTheScreenRecordingGrant();
};

void MeetingFlowTest::initTestCase() {
    verifyEmbeddedQmldir(QStringLiteral(MA_MEETING_FLOW_QML_IMPORT_DIR));
    qmlRegisterType<JobPoller>("MeetingAssistant", 1, 0, "JobPoller");
}

// The minutes opened right after generation had created_at 0, so the header lost
// its date and began with the separator dot, until the meeting was reopened.
void MeetingFlowTest::minutesKeepTheMeetingDateAfterGeneration() {
    App app;
    QVERIFY(app.start());

    QVERIFY(app.generateMinutes(app.sidecar.addMeeting(QStringLiteral("sync"), true)));

    QCOMPARE(app.page()->property("createdAt").toLongLong(), kCreatedAt);
}

// Qt keeps a layout's first zero-implicit height as its preferred height for good.
// This screen opens with the finished card up, so an empty job section used to
// keep the card's space above the minutes until the meeting was reopened.
void MeetingFlowTest::finishedCardLeavesNoGapAboveTheMinutes() {
    App app;
    QVERIFY(app.start());

    const QVariantMap meeting = app.sidecar.addMeeting(QStringLiteral("sync"), true);
    QVERIFY(app.generateMinutes(meeting));

    // The card is up, so the document really does start lower for now.
    QTRY_VERIFY(app.gapAboveDocument() > 0);

    QTRY_VERIFY_WITH_TIMEOUT(app.page()->property("jobEntry").isNull(), kSweepTimeoutMs);
    QTRY_COMPARE(app.gapAboveDocument(), qreal(0));

    // The same page opened fresh, for comparison.
    QMetaObject::invokeMethod(app.shell, "showDetail", Q_ARG(QVariant, meeting));
    QTRY_VERIFY(app.page()->property("hasProtocol").toBool());
    QTRY_COMPARE(app.gapAboveDocument(), qreal(0));
}

// An import queues a transcription that nothing told the UI about: it said
// "no minutes yet" and offered to start during the job, and kept saying so after
// it, so starting then cleared the saved transcript and ran Whisper again.
void MeetingFlowTest::importShowsProgressThenTheSavedTranscript() {
    App app;
    QVERIFY(app.start());
    const QString id = QStringLiteral("imported");

    app.importFile(QStringLiteral("/sandbox/imported.wav"));

    QTRY_VERIFY(app.isTracked(id));
    QTRY_COMPARE(app.pageName(), QStringLiteral("MeetingDetailScreen"));
    QTRY_VERIFY(app.sidecar.hits(QStringLiteral("/api/v1/meetings/") + id) > 0);
    QTest::qWait(100); // the screen's reaction to that reply, if it had any

    // Progress on screen, and no way to queue a second job over this one.
    QCOMPARE(app.pageName(), QStringLiteral("MeetingDetailScreen"));
    QTRY_VERIFY(app.visibleText(QStringLiteral("Whisper распознаёт речь…")) != nullptr);
    auto *header = app.page()->findChild<QQuickItem *>(QStringLiteral("detailHeader"));
    QVERIFY(header != nullptr);
    QTRY_COMPARE(header->y(), qreal(16)); // its top margin: nothing fills the page yet
    QVERIFY(app.visibleText(QStringLiteral("в работе")) != nullptr);
    QVERIFY(app.visibleText(kNoMinutesTitle) == nullptr);
    QVERIFY(app.visibleText(kGenerateMinutes) == nullptr);
    QVERIFY(app.sidecar.reprocessKinds().isEmpty());

    app.sidecar.finishJob(app.sidecar.liveJobOf(id));

    // The transcript is known everywhere as soon as the job ends.
    QTRY_VERIFY_WITH_TIMEOUT(app.listedWithTranscript(id), 5000);
    QTRY_COMPARE(app.pageName(), QStringLiteral("MeetingNoProtocolScreen"));
    QTRY_VERIFY(app.page()->property("hasTranscript").toBool());
    QVERIFY(app.visibleText(QStringLiteral("транскрипт готов")) != nullptr);

    // With the job gone the screen offers minutes, which reuse that transcript.
    QTRY_VERIFY_WITH_TIMEOUT(app.visibleText(kNoMinutesTitle) != nullptr, kSweepTimeoutMs);
    QVERIFY(app.visibleText(QStringLiteral(
                "Транскрипт уже готов. Запустите генерацию протокола — это займёт пару минут."))
            != nullptr);
    QVERIFY(app.clickUntil(kGenerateMinutes, [&app] {
        return app.pageName() == QLatin1String("GenerateProtocolScreen");
    }));
    QVERIFY(app.visibleText(QStringLiteral("Транскрипт готов")) != nullptr);
    QVERIFY(app.clickUntil(kGenerateMinutes, [&app] { return !app.sidecar.reprocessKinds().isEmpty(); }));
    QCOMPARE(app.sidecar.reprocessKinds(), QStringList{QStringLiteral("protocol")});
}

// With no minutes to show, the detail screen would be left blank once the failed
// job's card goes away.
void MeetingFlowTest::failedImportOffersToStartAgain() {
    App app;
    QVERIFY(app.start());
    const QString id = QStringLiteral("imported");

    app.importFile(QStringLiteral("/sandbox/imported.wav"));
    QTRY_VERIFY(app.isTracked(id));
    QTRY_COMPARE(app.pageName(), QStringLiteral("MeetingDetailScreen"));

    app.sidecar.failJob(app.sidecar.liveJobOf(id));

    QTRY_COMPARE_WITH_TIMEOUT(app.pageName(), QStringLiteral("MeetingNoProtocolScreen"), 5000);
    QVERIFY(!app.page()->property("hasTranscript").toBool());
    QTRY_VERIFY_WITH_TIMEOUT(app.visibleText(kNoMinutesTitle) != nullptr, kSweepTimeoutMs);
    QVERIFY(app.visibleText(kGenerateMinutes) != nullptr);
    QVERIFY(app.visibleText(QStringLiteral("нет транскрипта")) != nullptr);
}

// The "Из папки" dialog imports several files at once. A Request delivers only
// the reply to its latest call, so one shared Request lost the other files' jobs.
void MeetingFlowTest::everyFileOfABatchImportIsTracked() {
    App app;
    QVERIFY(app.start());

    QMetaObject::invokeMethod(app.shell, "showNewRecording");
    for (const char *path : {"/sandbox/first.wav", "/sandbox/second.wav"})
        QMetaObject::invokeMethod(app.store(), "importFile", Q_ARG(QVariant, QString::fromLatin1(path)),
                                  Q_ARG(QVariant, false), Q_ARG(QVariant, true));

    QTRY_VERIFY(app.isTracked(QStringLiteral("first")));
    QTRY_VERIFY(app.isTracked(QStringLiteral("second")));
}

// Only the job's own screen used to refresh the list, so leaving it for anything
// else left the sidebar saying "no transcript" for good.
void MeetingFlowTest::finishedJobUpdatesTheSidebarFromAnyScreen() {
    App app;
    QVERIFY(app.start());
    const QString id = QStringLiteral("imported");

    app.importFile(QStringLiteral("/sandbox/imported.wav"));
    QTRY_VERIFY(app.isTracked(id));
    QTRY_COMPARE(app.pageName(), QStringLiteral("MeetingDetailScreen"));

    QMetaObject::invokeMethod(app.shell, "showSettings");
    QTRY_COMPARE(app.pageName(), QStringLiteral("SettingsScreen"));

    app.sidecar.finishJob(app.sidecar.liveJobOf(id));

    QTRY_VERIFY_WITH_TIMEOUT(app.listedWithTranscript(id), 5000);
    QTRY_VERIFY(app.visibleText(QStringLiteral("транскрипт")) != nullptr);
}

// On macOS the sidecar named the system leg "System audio", and the Russian
// interface showed it as is. It now sends an empty label, which this screen
// names itself.
void MeetingFlowTest::recordingNamesTheSystemMixInTheInterfaceLanguage() {
    App app;
    QVERIFY(app.start());

    QMetaObject::invokeMethod(app.shell, "showNewRecording");
    QTRY_COMPARE(app.pageName(), QStringLiteral("NewRecordingScreen"));
    QMetaObject::invokeMethod(app.page(), "start", Q_ARG(QVariant, QStringLiteral("Планёрка")));

    QTRY_COMPARE(app.page()->property("st").toString(), QStringLiteral("recording"));
    QTRY_VERIFY(app.visibleTextContaining(
                    QStringLiteral("🎙 MacBook Pro Microphone   🔊 Системный звук"))
                != nullptr);
}

// The template field took its text only on Enter or when it lost focus, and a
// button click takes neither, so typing a template and clicking "Generate"
// generated with the default template.
void MeetingFlowTest::typedTemplateIsUsedWithoutPressingEnter() {
    App app;
    QVERIFY(app.start());

    const QVariantMap meeting = app.sidecar.addMeeting(QStringLiteral("sync"), true);
    QMetaObject::invokeMethod(app.shell, "showGenerate", Q_ARG(QVariant, meeting), Q_ARG(QVariant, false));
    QTRY_COMPARE(app.pageName(), QStringLiteral("GenerateProtocolScreen"));

    QQuickItem *field = nullptr;
    QTRY_VERIFY((field = findVisible(app.page(), [](QQuickItem *item) {
                     return item->property("placeholderText").toString() == QStringLiteral("По умолчанию");
                 })) != nullptr);
    QTest::mouseClick(app.quickWindow(), Qt::LeftButton, Qt::NoModifier,
                      field->mapToScene(QPointF(field->width() / 2, field->height() / 2)).toPoint());
    QTRY_VERIFY(field->hasActiveFocus());
    for (const char c : QByteArrayLiteral("Simple minutes"))
        QTest::keyClick(app.quickWindow(), c);
    QCOMPARE(field->property("text").toString(), QStringLiteral("Simple minutes"));

    QVERIFY(app.clickUntil(kGenerateMinutes, [&app] { return !app.sidecar.reprocessKinds().isEmpty(); }));
    QCOMPARE(app.sidecar.reprocessTemplates(), QStringList{QStringLiteral("Simple minutes")});
}

// After an update of the ad-hoc signed build macOS keeps the Screen Recording
// grant for the old binary. The dialog told the user to remove the app from the
// list and add it back, which did not help in the alpha.15 smoke run; resetting
// the grant did. The dialog now offers that reset.
void MeetingFlowTest::permissionDialogResetsTheScreenRecordingGrant() {
    App app;
    app.sidecar.deniesScreenCapture = true;
    QVERIFY(app.start());

    QMetaObject::invokeMethod(app.shell, "showNewRecording");
    QTRY_COMPARE(app.pageName(), QStringLiteral("NewRecordingScreen"));
    QMetaObject::invokeMethod(app.page(), "start", Q_ARG(QVariant, QStringLiteral("Планёрка")));

    QTRY_VERIFY(app.visibleText(QStringLiteral("Нужно разрешение macOS")) != nullptr);
    QVERIFY(app.visibleTextContaining(QStringLiteral("кнопкой «−»")) == nullptr);
    QVERIFY(app.clickUntil(QStringLiteral("Сбросить разрешение"),
                           [&app] { return app.permission.resets > 0; }));
    QCOMPARE(app.permission.resets, 1);
    QTRY_VERIFY(app.visibleTextContaining(QStringLiteral("Разрешение сброшено.")) != nullptr);
    QVERIFY(app.visibleText(QStringLiteral("Сбросить разрешение")) == nullptr);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQuickStyle::setStyle(QStringLiteral("Fusion")); // as main.cpp does
    MeetingFlowTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_meeting_flow.moc"
