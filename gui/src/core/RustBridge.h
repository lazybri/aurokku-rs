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
    void scanAsync(int vid, int pid);

    /// Run one protocol operation described by a JSON request.
    void runAsync(const QJsonObject &request, const QString &label);

    bool busy() const { return m_busy; }

signals:
    void scanStarted();
    void scanFinished(const QJsonArray &devices);
    void scanFailed(const QString &message);

    void taskStarted(const QString &label);
    void taskFinished(const QString &label, bool ok, const QJsonObject &data,
                      const QString &errorKind, const QString &errorMessage);

    void logLine(const QString &line);

private:
    void markBusy(bool busy);

    QThread *m_thread = nullptr;
    RustWorker *m_worker = nullptr;
    bool m_busy = false;
};
