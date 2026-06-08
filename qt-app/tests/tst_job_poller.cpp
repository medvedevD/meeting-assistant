#include "ApiClient.h"
#include "JobPoller.h"

#include <QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QtTest>

class JobPollerTest : public QObject {
    Q_OBJECT

private slots:
    void decodeSseFrameParsesEventAndData();
    void decodeSseFrameRejectsKeepaliveAndGarbage();
    void sseStreamEmitsProgressThenTerminal();
    void terminalDoneResponseEmitsPayloadAndStops();
    void pendingThenDoneKeepsPollingUntilTerminal();
};

// ── pure wire-parsing contract (no socket) ───────────────────────────────────

void JobPollerTest::decodeSseFrameParsesEventAndData() {
    const QByteArray frame =
        "event: progress\n"
        "data: {\"status\":\"running\",\"progress\":{\"percent\":42}}";
    QString event;
    QVariant json;
    QVERIFY(JobPoller::decodeSseFrame(frame, event, json));
    QCOMPARE(event, QStringLiteral("progress"));
    const QVariantMap map = json.toMap();
    QCOMPARE(map.value(QStringLiteral("status")).toString(),
             QStringLiteral("running"));
    QCOMPARE(map.value(QStringLiteral("progress")).toMap()
                 .value(QStringLiteral("percent")).toInt(),
             42);
}

void JobPollerTest::decodeSseFrameRejectsKeepaliveAndGarbage() {
    QString event;
    QVariant json;
    // Comment-only keepalive frame → no data.
    QVERIFY(!JobPoller::decodeSseFrame(": keep-alive", event, json));
    // event with no data line → nothing to decode.
    QVERIFY(!JobPoller::decodeSseFrame("event: status", event, json));
    // data present but not valid JSON.
    QVERIFY(!JobPoller::decodeSseFrame("data: not-json", event, json));
}

// ── SSE happy path: stream drives progress + terminal, no polling ────────────

void JobPollerTest::sseStreamEmitsProgressThenTerminal() {
    QTcpServer server;
    QVERIFY(server.listen(QHostAddress::LocalHost, 0));

    int pollRequests = 0; // GET /api/v1/jobs/:id (the fallback path)

    QObject::connect(&server, &QTcpServer::newConnection, &server,
                     [&server, &pollRequests]() {
        QTcpSocket *sock = server.nextPendingConnection();
        QObject::connect(sock, &QTcpSocket::readyRead, sock,
                         [sock, &pollRequests]() {
            if (!sock->canReadLine())
                return;
            const QByteArray reqLine = sock->readLine();
            if (!reqLine.contains("/events")) {
                ++pollRequests; // a fallback poll we don't expect to happen
                return;
            }
            // Two SSE frames (compact JSON, no internal blank lines): a live
            // `progress` update followed by a terminal `status` done.
            const QByteArray stream =
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: text/event-stream\r\n"
                "Connection: close\r\n"
                "\r\n"
                "event: progress\n"
                "data: {\"id\":\"job-sse\",\"meeting_id\":\"m1\",\"kind\":\"transcribe\","
                "\"status\":\"running\",\"progress\":{\"stage\":\"transcribing\","
                "\"sub\":\"\",\"percent\":40}}\n\n"
                "event: status\n"
                "data: {\"id\":\"job-sse\",\"meeting_id\":\"m1\",\"kind\":\"transcribe\","
                "\"status\":\"done\",\"progress\":null,\"error_class\":null}\n\n";
            sock->write(stream);
            sock->disconnectFromHost();
        });
        QObject::connect(sock, &QTcpSocket::disconnected, sock,
                         &QObject::deleteLater);
    });

    ApiClient api;
    api.configure(
        QStringLiteral("http://127.0.0.1:%1").arg(server.serverPort()),
        QStringLiteral("test-token"));

    JobPoller poller;
    poller.setApi(&api);
    poller.setIntervalMs(10);
    poller.setJobId(QStringLiteral("job-sse"));

    QSignalSpy jobUpdated(&poller, &JobPoller::jobUpdated);
    QSignalSpy statusChanged(&poller, &JobPoller::statusChanged);

    poller.start();

    QTRY_VERIFY_WITH_TIMEOUT(jobUpdated.count() >= 2, 5000);
    QTRY_VERIFY_WITH_TIMEOUT(!poller.isActive(), 5000);

    // First frame: a live progress update (status running, percent carried).
    QCOMPARE(jobUpdated.at(0).at(0).toString(), QStringLiteral("running"));
    const QVariantMap running = jobUpdated.at(0).at(1).toMap();
    QCOMPARE(running.value(QStringLiteral("progress")).toMap()
                 .value(QStringLiteral("percent")).toInt(),
             40);
    // Last frame: terminal done.
    QCOMPARE(jobUpdated.last().at(0).toString(), QStringLiteral("done"));
    // running → done is two distinct status transitions.
    QCOMPARE(statusChanged.count(), 2);
    // The stream carried everything; the polling fallback never ran.
    QCOMPARE(pollRequests, 0);
}

// ── polling fallback: when the stream is refused, fall back to GET /jobs ──────

void JobPollerTest::terminalDoneResponseEmitsPayloadAndStops() {
    QTcpServer server;
    QVERIFY(server.listen(QHostAddress::LocalHost, 0));

    const QByteArray body = R"json({
        "id":"job-done",
        "meeting_id":"meeting-1",
        "kind":"regenerate_protocol",
        "status":"done",
        "attempts":1,
        "last_error":null,
        "error_class":null,
        "progress":null,
        "created_at":1,
        "updated_at":2
    })json";

    QObject::connect(&server, &QTcpServer::newConnection, &server, [&server, body]() {
        QTcpSocket *sock = server.nextPendingConnection();
        QObject::connect(sock, &QTcpSocket::readyRead, sock, [sock, body]() {
            if (!sock->canReadLine())
                return;
            const QByteArray reqLine = sock->readLine();
            // Refuse the SSE stream so the poller falls back to polling.
            if (reqLine.contains("/events")) {
                sock->write("HTTP/1.1 404 Not Found\r\n"
                            "Content-Length: 0\r\n"
                            "Connection: close\r\n\r\n");
                sock->disconnectFromHost();
                return;
            }
            const QByteArray response =
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: application/json\r\n"
                "Content-Length: " + QByteArray::number(body.size()) + "\r\n"
                "Connection: close\r\n"
                "\r\n" + body;
            sock->write(response);
            sock->disconnectFromHost();
        });
        QObject::connect(sock, &QTcpSocket::disconnected, sock, &QObject::deleteLater);
    });

    ApiClient api;
    api.configure(
        QStringLiteral("http://127.0.0.1:%1").arg(server.serverPort()),
        QStringLiteral("test-token"));

    JobPoller poller;
    poller.setApi(&api);
    poller.setIntervalMs(10);
    poller.setJobId(QStringLiteral("job-done"));

    QSignalSpy statusChanged(&poller, &JobPoller::statusChanged);
    QSignalSpy jobUpdated(&poller, &JobPoller::jobUpdated);
    QSignalSpy activeChanged(&poller, &JobPoller::activeChanged);

    poller.start();

    QTRY_COMPARE_WITH_TIMEOUT(jobUpdated.count(), 1, 5000);
    QTRY_VERIFY_WITH_TIMEOUT(!poller.isActive(), 5000);

    QCOMPARE(statusChanged.count(), 1);
    const QList<QVariant> statusArgs = statusChanged.takeFirst();
    QCOMPARE(statusArgs.size(), 2);
    QCOMPARE(statusArgs.at(0).toString(), QStringLiteral("done"));
    const QVariantMap statusJob = statusArgs.at(1).toMap();
    QCOMPARE(statusJob.value(QStringLiteral("id")).toString(), QStringLiteral("job-done"));
    QCOMPARE(statusJob.value(QStringLiteral("status")).toString(), QStringLiteral("done"));

    const QList<QVariant> updateArgs = jobUpdated.takeFirst();
    QCOMPARE(updateArgs.size(), 2);
    QCOMPARE(updateArgs.at(0).toString(), QStringLiteral("done"));
    const QVariantMap updateJob = updateArgs.at(1).toMap();
    QCOMPARE(updateJob.value(QStringLiteral("status")).toString(), QStringLiteral("done"));

    QVERIFY(activeChanged.count() >= 2);
}

void JobPollerTest::pendingThenDoneKeepsPollingUntilTerminal() {
    QTcpServer server;
    QVERIFY2(server.listen(QHostAddress::LocalHost, 0),
             qPrintable(server.errorString()));

    int requestCount = 0;

    QObject::connect(&server, &QTcpServer::newConnection, &server, [&server, &requestCount]() {
        QTcpSocket *sock = server.nextPendingConnection();
        QObject::connect(sock, &QTcpSocket::readyRead, sock, [sock, &requestCount]() {
            if (!sock->canReadLine())
                return;
            const QByteArray reqLine = sock->readLine();
            // Refuse the SSE stream so the poller falls back to polling, then
            // exercise the pending → done polling sequence.
            if (reqLine.contains("/events")) {
                sock->write("HTTP/1.1 404 Not Found\r\n"
                            "Content-Length: 0\r\n"
                            "Connection: close\r\n\r\n");
                sock->disconnectFromHost();
                return;
            }

            ++requestCount;
            const bool done = requestCount >= 2;
            const QByteArray body = done
                ? QByteArrayLiteral(R"json({
                    "id":"job-two-step",
                    "meeting_id":"meeting-1",
                    "kind":"regenerate_protocol",
                    "status":"done",
                    "attempts":1,
                    "last_error":null,
                    "error_class":null,
                    "progress":null,
                    "created_at":1,
                    "updated_at":3
                })json")
                : QByteArrayLiteral(R"json({
                    "id":"job-two-step",
                    "meeting_id":"meeting-1",
                    "kind":"regenerate_protocol",
                    "status":"pending",
                    "attempts":0,
                    "last_error":null,
                    "error_class":null,
                    "progress":{"stage":"queued","sub":"В очереди","percent":0},
                    "created_at":1,
                    "updated_at":2
                })json");

            const QByteArray response =
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: application/json\r\n"
                "Content-Length: " + QByteArray::number(body.size()) + "\r\n"
                "Connection: close\r\n"
                "\r\n" + body;
            sock->write(response);
            sock->disconnectFromHost();
        });
        QObject::connect(sock, &QTcpSocket::disconnected, sock, &QObject::deleteLater);
    });

    ApiClient api;
    api.configure(
        QStringLiteral("http://127.0.0.1:%1").arg(server.serverPort()),
        QStringLiteral("test-token"));

    JobPoller poller;
    poller.setApi(&api);
    poller.setIntervalMs(10);
    poller.setJobId(QStringLiteral("job-two-step"));

    QSignalSpy jobUpdated(&poller, &JobPoller::jobUpdated);

    poller.start();

    QTRY_VERIFY_WITH_TIMEOUT(jobUpdated.count() >= 2, 5000);
    QTRY_VERIFY_WITH_TIMEOUT(!poller.isActive(), 5000);

    QCOMPARE(requestCount, 2);
    QCOMPARE(jobUpdated.at(0).at(0).toString(), QStringLiteral("pending"));
    QCOMPARE(jobUpdated.at(1).at(0).toString(), QStringLiteral("done"));
}

QTEST_MAIN(JobPollerTest)

#include "tst_job_poller.moc"
