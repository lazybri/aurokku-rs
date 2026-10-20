#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QString>

/// Lives on the dedicated USB thread and performs the blocking FFI calls.
class RustWorker : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;

public slots:
    void scan(int vid, int pid);
    void runTask(const QJsonObject &request, const QString &label);

signals:
    void scanStarted();
    void scanFinished(const QJsonArray &devices);
    void scanFailed(const QString &message);

    void taskStarted(const QString &label);
    void taskFinished(const QString &label, bool ok, const QJsonObject &data,
                      const QString &errorKind, const QString &errorMessage);

    void logLine(const QString &line);

private:
    /// C callback handed to `auroku_run_task`; forwards one diagnostic line.
    static void logTrampoline(const char *line, void *userData);
};
