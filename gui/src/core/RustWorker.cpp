#include "RustWorker.h"

#include <QByteArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QJsonValue>

#include "FfiApi.h"

namespace {

/// Decoded `{ "ok": ..., "data": ..., "kind": ..., "message": ... }` envelope.
struct Envelope {
    bool ok = false;
    QJsonValue data;
    QString kind = QStringLiteral("Error");
    QString message;

    QString describe() const
    {
        if (kind.isEmpty())
            return message;
        if (message.isEmpty())
            return kind;
        return QStringLiteral("%1: %2").arg(kind, message);
    }
};

Envelope parseEnvelope(char *raw)
{
    Envelope envelope;
    if (raw == nullptr) {
        envelope.message = QStringLiteral("backend returned a null pointer");
        return envelope;
    }
    // Always release the Rust-owned string, even if parsing fails.
    const QByteArray bytes(raw);
    auroku_string_free(raw);

    QJsonParseError error{};
    const QJsonDocument document = QJsonDocument::fromJson(bytes, &error);
    if (error.error != QJsonParseError::NoError || !document.isObject()) {
        envelope.message = QStringLiteral("invalid backend response: %1").arg(error.errorString());
        return envelope;
    }

    const QJsonObject object = document.object();
    envelope.ok = object.value(QStringLiteral("ok")).toBool(false);
    envelope.data = object.value(QStringLiteral("data"));
    envelope.kind = object.value(QStringLiteral("kind")).toString(envelope.kind);
    envelope.message = object.value(QStringLiteral("message")).toString();
    return envelope;
}

} // namespace

void RustWorker::scan(int vid, int pid)
{
    emit scanStarted();
    const Envelope envelope = parseEnvelope(auroku_scan_devices(vid, pid));
    if (!envelope.ok) {
        emit scanFailed(envelope.describe());
        return;
    }
    emit scanFinished(envelope.data.toArray());
}

void RustWorker::runTask(const QJsonObject &request, const QString &label)
{
    emit taskStarted(label);
    const QByteArray json = QJsonDocument(request).toJson(QJsonDocument::Compact);
    const Envelope envelope =
        parseEnvelope(auroku_run_task(json.constData(), &RustWorker::logTrampoline, this));
    emit taskFinished(label, envelope.ok, envelope.data.toObject(), envelope.kind, envelope.message);
}

void RustWorker::logTrampoline(const char *line, void *userData)
{
    if (line == nullptr || userData == nullptr)
        return;
    auto *worker = static_cast<RustWorker *>(userData);
    // Emitted from the USB thread; the bridge connection is queued.
    emit worker->logLine(QString::fromUtf8(line));
}
