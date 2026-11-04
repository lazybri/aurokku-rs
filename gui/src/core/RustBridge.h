#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QObject>
#include <QString>

class QThread;
class RustWorker;

/// Qt facade over the Rust FFI bridge.
///
/// The GUI thread only talks to this object. Every USB call (including scans)
/// runs on a dedicated worker thread, and results are delivered as queued
/// signals, so the window never blocks on libusb transfers or timeouts.
class RustBridge : public QObject {
    Q_OBJECT
public:
    explicit RustBridge(QObject *parent = nullptr);
    ~RustBridge() override;

    /// Enumerate devices; pass -1 to skip a filter.
    ///
    /// Every async entry point returns false when the single USB worker is
    /// already busy. It never silently drops the request: the caller can react
    /// to the rejection (the GUI shows a toast) and the `taskRejected` signal
    /// also carries the reason.
    bool scanAsync(int vid, int pid);

    /// Run one protocol operation described by a JSON request.
    bool runAsync(const QJsonObject &request, const QString &label);

    // WinUSB driver provisioning wrappers (see FfiApi.h).
    bool driverStatusAsync();
    bool driverInstallAsync(int vid, int pid, bool restoreOnExit);
    bool driverRestoreAsync(int vid, int pid);

    bool busy() const { return m_busy; }

signals:
    void scanStarted();
    void scanFinished(const QJsonArray &devices);
    void scanFailed(const QString &message);

    void taskStarted(const QString &label);
    void taskFinished(const QString &label, bool ok, const QJsonObject &data,
                      const QString &errorKind, const QString &errorMessage);

    void driverStarted(const QString &op);
    void driverFinished(const QString &op, bool ok, const QJsonObject &data,
                        const QString &errorKind, const QString &errorMessage);

    /// Emitted when an async request is refused because the worker is busy.
    void taskRejected(const QString &reason);

    void logLine(const QString &line);

private:
    /// Mark the worker busy, or return false when it already is.
    bool tryMarkBusy();
    void markBusy(bool busy);

    QThread *m_thread = nullptr;
    RustWorker *m_worker = nullptr;
    bool m_busy = false;
};
