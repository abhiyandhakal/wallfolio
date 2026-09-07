#pragma once
#include <QObject>
#include <QVariant>
class Client : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
public:
    using QObject::QObject;
    bool busy() const { return m_busy; }
    Q_INVOKABLE void request(const QString &method, const QVariantMap &params);
    Q_INVOKABLE QString fileUrl(const QString &path) const;
    Q_INVOKABLE void startDaemon();
signals:
    void busyChanged();
    void completed(QString method, QVariant result);
    void failed(QString message);
private:
    bool m_busy = false;
};
