#include "ApiClient.h"
#include "JobPoller.h"
#include "PathUtils.h"
#include "SidecarManager.h"

#include <QGuiApplication>
#include <QFont>
#include <QFontDatabase>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QtQml>

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("Meeting Assistant"));
    QGuiApplication::setOrganizationName(QStringLiteral("meeting-assistant"));
    QGuiApplication::setApplicationVersion(QStringLiteral("0.1.0"));

    auto loadFont = [](const QString &path) -> QString {
        const int id = QFontDatabase::addApplicationFont(path);
        if (id < 0) {
            qWarning("Failed to load bundled font: %s", qPrintable(path));
            return {};
        }
        const QStringList families = QFontDatabase::applicationFontFamilies(id);
        return families.isEmpty() ? QString{} : families.first();
    };
    const QString uiFont = loadFont(QStringLiteral(":/fonts/Geist-Variable.ttf"));
    loadFont(QStringLiteral(":/fonts/Newsreader-Variable.ttf"));
    loadFont(QStringLiteral(":/fonts/Newsreader-Italic-Variable.ttf"));
    loadFont(QStringLiteral(":/fonts/JetBrainsMono-Variable.ttf"));
    if (!uiFont.isEmpty()) {
        QFont appFont(uiFont);
        QGuiApplication::setFont(appFont);
        qInfo("Application UI font: %s", qPrintable(uiFont));
    }

    // Fusion enforcement (suspenders; the compiled-in :/qtquickcontrols2.conf
    // is the belt). Must be set after QGuiApplication and before the QML engine
    // loads. Basic/Material/Universal are forbidden in v1 and their plugins are
    // not linked/shipped.
    QQuickStyle::setStyle(QStringLiteral("Fusion"));
    if (QQuickStyle::name() != QStringLiteral("Fusion")) {
        qFatal("Fusion style not active (got '%s') — refusing to start",
               qPrintable(QQuickStyle::name()));
    }
    qInfo("Qt Quick Controls style: %s", qPrintable(QQuickStyle::name()));

    SidecarManager sidecar;
    ApiClient api;
    PathUtils pathUtils;

    // Wire the client the moment the sidecar is Ready (handshake + version
    // gate + /health all passed).
    QObject::connect(&sidecar, &SidecarManager::ready, &api,
                     &ApiClient::configure);

    // Reap the child on every exit path (clean quit, last window closed).
    QObject::connect(&app, &QGuiApplication::aboutToQuit, &sidecar,
                     [&sidecar]() { sidecar.shutdown(); });

    qmlRegisterType<JobPoller>("MeetingAssistant", 1, 0, "JobPoller");
    // Registered uncreatable so QML can reference the SidecarManager.State
    // enum; the instance itself is injected as the `sidecar` context property.
    qmlRegisterUncreatableType<SidecarManager>(
        "MeetingAssistant", 1, 0, "SidecarManager",
        QStringLiteral("Provided by C++ as the 'sidecar' context property"));

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("sidecar"),
                                             &sidecar);
    engine.rootContext()->setContextProperty(QStringLiteral("api"), &api);
    engine.rootContext()->setContextProperty(QStringLiteral("pathUtils"),
                                             &pathUtils);
    engine.rootContext()->setContextProperty(QStringLiteral("controlsStyle"),
                                              QQuickStyle::name());
    // qt-redesign Phase-1 dev affordance: MEETY_THEME_GALLERY=1 swaps the root
    // view for the Theme token gallery (palette + typography) for sign-off.
    engine.rootContext()->setContextProperty(
        QStringLiteral("themeGalleryEnabled"),
        qEnvironmentVariableIsSet("MEETY_THEME_GALLERY"));
    // Dev-only: FIRST_RUN=1 ./run-qt.sh forces the welcome/first-run surface
    // while the script also isolates XDG data/config from the real profile.
    engine.rootContext()->setContextProperty(
        QStringLiteral("firstRunPreviewEnabled"),
        qEnvironmentVariableIsSet("FIRST_RUN"));

    QObject::connect(
        &engine, &QQmlApplicationEngine::objectCreationFailed, &app,
        []() { QCoreApplication::exit(-1); }, Qt::QueuedConnection);
    engine.loadFromModule("MeetingAssistant", "Main");
    if (engine.rootObjects().isEmpty())
        return -1;

    sidecar.start();
    return app.exec();
}
