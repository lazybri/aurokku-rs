#include "RustBridge.h"

#include <QMetaObject>
#include <QThread>

#include "RustWorker.h"

namespace {

/// Reason reported through `taskRejected` when the USB thread is occupied.
constexpr auto kBusyReason = "busy";

} // namespace

RustBridge::RustBridge(QObject *parent)
    : QObject(parent)
{
    m_thread = new QThread(this);
    m_thread->setObjectName(QStringLiteral("auroku-usb"));

    m_worker = new RustWorker;
    m_worker->moveToThread(m_thread);

    connect(m_thread, &QThread::finished, m_worker, &QObject::deleteLater);

    connect(m_worker, &RustWorker::scanStarted, this, &RustBridge::scanStarted);
    connect(m_worker, &RustWorker::logLine, this, &RustBridge::logLine);
    connect(m_worker, &RustWorker::taskStarted, this, &RustBridge::taskStarted);
    connect(m_worker, &RustWorker::driverStarted, this, &RustBridge::driverStarted);

    // Every completion clears the busy flag before the result is delivered, so
    // a handler may immediately queue the next request (e.g. driver install
    // followed by the automatic retry).
    connect(m_worker, &RustWorker::scanFinished, this, [this](const QJsonArray &devices) {
        markBusy(false);
        emit scanFinished(devices);
    });
    connect(m_worker, &RustWorker::scanFailed, this, [this](const QString &message) {
        markBusy(false);
        emit scanFailed(message);
    });
    connect(m_worker, &RustWorker::taskFinished, this,
            [this](const QString &label, bool ok, const QJsonObject &data, const QString &kind,
                   const QString &message) {
                markBusy(false);
                emit taskFinished(label, ok, data, kind, message);
            });
    connect(m_worker, &RustWorker::driverFinished, this,
            [this](const QString &op, bool ok, const QJsonObject &data, const QString &kind,
                   const QString &message) {
                markBusy(false);
                emit driverFinished(op, ok, data, kind, message);
            });

    m_thread->start();
}

RustBridge::~RustBridge()
{
    if (m_thread == nullptr)
        return;
    m_thread->quit();
    m_thread->wait();
}

bool RustBridge::scanAsync(int vid, int pid)
{
    if (m_worker == nullptr)
        return false;
    if (!tryMarkBusy()) {
        emit taskRejected(QString::fromLatin1(kBusyReason));
        return false;
    }
    QMetaObject::invokeMethod(
        m_worker, [worker = m_worker, vid, pid] { worker->scan(vid, pid); },
        Qt::QueuedConnection);
    return true;
}

bool RustBridge::runAsync(const QJsonObject &request, const QString &label)
{
    if (m_worker == nullptr)
        return false;
    if (!tryMarkBusy()) {
        emit taskRejected(QString::fromLatin1(kBusyReason));
        return false;
    }
    QMetaObject::invokeMethod(
        m_worker, [worker = m_worker, request, label] { worker->runTask(request, label); },
        Qt::QueuedConnection);
    return true;
}

bool RustBridge::driverStatusAsync()
{
    if (m_worker == nullptr)
        return false;
    // Internal plumbing: when the worker is busy the caller just retries later,
    // so no taskRejected toast is emitted for it.
    if (!tryMarkBusy())
        return false;
    QMetaObject::invokeMethod(m_worker, [worker = m_worker] { worker->driverStatus(); },
                              Qt::QueuedConnection);
    return true;
}

bool RustBridge::driverInstallAsync(int vid, int pid, bool restoreOnExit)
{
    if (m_worker == nullptr)
        return false;
    if (!tryMarkBusy()) {
        emit taskRejected(QString::fromLatin1(kBusyReason));
        return false;
    }
    QMetaObject::invokeMethod(
        m_worker,
        [worker = m_worker, vid, pid, restoreOnExit] {
            worker->driverInstall(vid, pid, restoreOnExit);
        },
        Qt::QueuedConnection);
    return true;
}

bool RustBridge::driverRestoreAsync(int vid, int pid)
{
    if (m_worker == nullptr)
        return false;
    if (!tryMarkBusy()) {
        emit taskRejected(QString::fromLatin1(kBusyReason));
        return false;
    }
    QMetaObject::invokeMethod(
        m_worker, [worker = m_worker, vid, pid] { worker->driverRestore(vid, pid); },
        Qt::QueuedConnection);
    return true;
}

bool RustBridge::tryMarkBusy()
{
    if (m_busy)
        return false;
    m_busy = true;
    return true;
}

void RustBridge::markBusy(bool busy)
{
    m_busy = busy;
}
