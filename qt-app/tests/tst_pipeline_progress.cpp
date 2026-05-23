#include "ApiClient.h"
#include "JobPoller.h"

#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QtTest>

class PipelineProgressTest : public QObject {
    Q_OBJECT

private slots:
    void pendingThenDoneEmitsFinishedFromQml();
};

void PipelineProgressTest::pendingThenDoneEmitsFinishedFromQml() {
    QTcpServer server;
    QVERIFY2(server.listen(QHostAddress::LocalHost, 0),
             qPrintable(server.errorString()));

    int requestCount = 0;
    QObject::connect(&server, &QTcpServer::newConnection, &server, [&server, &requestCount]() {
        QTcpSocket *sock = server.nextPendingConnection();
        QObject::connect(sock, &QTcpSocket::readyRead, sock, [sock, &requestCount]() {
            if (!sock->canReadLine())
                return;

            ++requestCount;
            const bool done = requestCount >= 2;
            const QByteArray body = done
                ? QByteArrayLiteral(R"json({
                    "id":"job-qml",
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
                    "id":"job-qml",
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

    qmlRegisterType<JobPoller>("MeetingAssistant", 1, 0, "JobPoller");

    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("api"), &api);
    engine.addImportPath(QStringLiteral(MA_PIPELINE_QML_IMPORT_DIR));

    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(MA_PIPELINE_QML_FILE)));
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    std::unique_ptr<QObject> root(component.create());
    QVERIFY2(root != nullptr, qPrintable(component.errorString()));

    QSignalSpy finished(root.get(), SIGNAL(finished(QString,QVariant)));
    QVERIFY(finished.isValid());

    root->setProperty("apiClient", QVariant::fromValue(static_cast<QObject *>(&api)));
    root->setProperty("jobId", QStringLiteral("job-qml"));

    QTRY_COMPARE_WITH_TIMEOUT(finished.count(), 1, 5000);
    QCOMPARE(finished.at(0).at(0).toString(), QStringLiteral("done"));
    QVERIFY(requestCount >= 2);
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    PipelineProgressTest tc;
    return QTest::qExec(&tc, argc, argv);
}

#include "tst_pipeline_progress.moc"
