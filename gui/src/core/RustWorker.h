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

    // WinUSB driver provisioning (Windows); the other platforms report a
    // DriverError envelope, so the C++ side never needs platform ifdefs.
    void driverStatus();
    void driverInstall(int vid, int pid, bool restoreOnExit);
    void driverRestore(int vid, int pid);

signals:
    void scanStarted();
    void scanFinished(const QJsonArray &devices);
    void scanFailed(const QString &message);

    void taskStarted(const QString &label);
    void taskFinished(const QString &label, bool ok, const QJsonObject &data,
                      const QString &errorKind, const QString &errorMessage);

    /// `op` is "status", "install" or "restore".
    void driverStarted(const QString &op);
    void driverFinished(const QString &op, bool ok, const QJsonObject &data,
                        const QString &errorKind, const QString &errorMessage);

    void logLine(const QString &line);

private:
    /// C callback handed to `auroku_run_task`; forwards one diagnostic line.
    static void logTrampoline(const char *line, void *userData);
};
