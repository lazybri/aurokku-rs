#include "RustBridge.h"

#include <QMetaObject>
#include <QThread>

#include "RustWorker.h"

RustBridge::RustBridge(QObject *parent)
    : QObject(parent)
{
    m_thread = new QThread(this);
    m_thread->setObjectName(QStringLiteral("auroku-usb"));

    m_worker = new RustWorker;
    m_worker->moveToThread(m_thread);

    connect(m_thread, &QThread::finished, m_worker, &QObject::deleteLater);

    connect(m_worker, &RustWorker::scanStarted, this, &RustBridge::scanStarted);
    connect(m_worker, &RustWorker::scanFinished, this, &RustBridge::scanFinished);
    connect(m_worker, &RustWorker::scanFailed, this, &RustBridge::scanFailed);
    connect(m_worker, &RustWorker::taskStarted, this, &RustBridge::taskStarted);
    connect(m_worker, &RustWorker::logLine, this, &RustBridge::logLine);

    connect(m_worker, &RustWorker::taskFinished, this,
            [this](const QString &label, bool ok, const QJsonObject &data, const QString &kind,
                   const QString &message) {
                m_busy = false;
                emit taskFinished(label, ok, data, kind, message);
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

void RustBridge::scanAsync(int vid, int pid)
{
    if (m_worker == nullptr)
        return;
    QMetaObject::invokeMethod(
        m_worker, [worker = m_worker, vid, pid] { worker->scan(vid, pid); },
        Qt::QueuedConnection);
}

void RustBridge::runAsync(const QJsonObject &request, const QString &label)
{
    if (m_worker == nullptr || m_busy)
        return;
    markBusy(true);
    QMetaObject::invokeMethod(
        m_worker, [worker = m_worker, request, label] { worker->runTask(request, label); },
        Qt::QueuedConnection);
}

void RustBridge::markBusy(bool busy)
{
    m_busy = busy;
}
