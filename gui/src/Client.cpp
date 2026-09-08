#include "Client.h"
#include <QLocalSocket>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QTimer>
#include <QUrl>
#include <QDir>
#include <QProcess>
#include <QCoreApplication>
#include <QFileInfo>
#include <memory>
QString Client::fileUrl(const QString &path) const { return QUrl::fromLocalFile(path).toString(); }
void Client::startDaemon() {
    QString executable = QCoreApplication::applicationDirPath() + "/wallfoliod";
    if (!QFileInfo::exists(executable)) executable = "wallfoliod";
    if (!QProcess::startDetached(executable, {})) emit failed("Cannot start wallfoliod. Add it to PATH or run it in a terminal.");
}
void Client::request(const QString &method, const QVariantMap &params) {
    if (m_busy) return;
    m_busy = true; emit busyChanged();
    auto socket = new QLocalSocket(this);
    auto timer = new QTimer(socket);
    timer->setSingleShot(true);
    auto buffer = std::make_shared<QByteArray>();
    auto done = std::make_shared<bool>(false);
    auto finish = [this,socket,timer,done]() {
        if (*done) return false;
        *done = true; timer->stop(); socket->abort(); socket->deleteLater();
        m_busy = false; emit busyChanged(); return true;
    };
    connect(timer,&QTimer::timeout,this,[this,finish]() { if (finish()) emit failed("Request timed out. Check the daemon and try again."); });
    connect(socket,&QLocalSocket::errorOccurred,this,[this,socket,finish](auto error) {
        const auto message = socket->errorString();
        if (!finish()) return;
        if (error == QLocalSocket::ServerNotFoundError || error == QLocalSocket::ConnectionRefusedError)
            emit unavailable(message);
        else emit failed(message);
    });
    connect(socket,&QLocalSocket::connected,this,[socket,method,params]() {
        socket->write(QJsonDocument(QJsonObject{{"version",1},{"method",method},{"params",QJsonObject::fromVariantMap(params)}}).toJson(QJsonDocument::Compact) + '\n');
    });
    connect(socket,&QLocalSocket::readyRead,this,[this,socket,buffer,finish,method]() {
        buffer->append(socket->readAll());
        if (buffer->size() > 1024*1024) { if(finish()) emit failed("Daemon response exceeded the size limit."); return; }
        auto newline = buffer->indexOf('\n'); if (newline < 0) return;
        QJsonParseError error;
        auto document = QJsonDocument::fromJson(buffer->left(newline), &error);
        if (!finish()) return;
        if (error.error != QJsonParseError::NoError || !document.isObject()) { emit failed("Invalid daemon response."); return; }
        auto object = document.object();
        if (!object["ok"].toBool()) emit failed(object["error"].toString());
        else emit completed(method,object["result"].toVariant());
    });
    QString path = qEnvironmentVariable("WALLFOLIO_SOCKET");
    if (path.isEmpty()) {
        auto base = qEnvironmentVariable("XDG_RUNTIME_DIR");
        if (base.isEmpty()) base = QDir::homePath() + "/.cache";
        path = base + "/wallfolio/wallfoliod.sock";
    }
    timer->start(180000);
    socket->connectToServer(path);
}
