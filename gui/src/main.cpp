#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include "Client.h"
int main(int argc, char **argv) {
    QGuiApplication app(argc, argv);
    QCoreApplication::setApplicationName("Wallfolio");
    QCoreApplication::setOrganizationName("Wallfolio");
    QQuickStyle::setStyle("Basic");
    Client client;
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("client", &client);
    engine.load(QUrl("qrc:/Wallfolio/qml/Main.qml"));
    if (engine.rootObjects().isEmpty()) return 1;
    const auto screenshot = qEnvironmentVariable("WALLFOLIO_SCREENSHOT");
    if (!screenshot.isEmpty()) {
        QTimer::singleShot(1200, &app, [&]() {
            auto window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
            app.exit(window && window->grabWindow().save(screenshot) ? 0 : 2);
        });
    }
    return app.exec();
}
