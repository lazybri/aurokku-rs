#pragma once

#include <QElapsedTimer>
#include <QJsonObject>
#include <QJsonValue>
#include <QString>
#include <QVector>

/// Semantic colour role used by chips, banners and metrics.
enum class Tone { Neutral, Primary, Success, Warning, Error, Tertiary };

/// One USB device discovered by `auroku_scan_devices`.
struct DeviceInfo {
    quint16 vid = 0;
    quint16 pid = 0;
    int bus = 0;
    int address = 0;
    QString manufacturer;
    QString product;
    QString serial;
    QString id;
    QString display;

    static DeviceInfo fromJson(const QJsonValue &value);
};

/// One console line, timestamped relative to application start.
struct LogLine {
    double at = 0.0;
    QString text;
};

/// All UI state that is not owned by a single widget.
struct AppState {
    // Device form -------------------------------------------------------------
    QString vidText = QStringLiteral("0482");
    QString pidText = QStringLiteral("0a5c");
    QString snText;
    QString interfaceText;
    QString epOutText = QStringLiteral("04");
    QString epInText = QStringLiteral("84");
    bool verbose = false;
    bool type2 = false;

    // Discovery ---------------------------------------------------------------
    QVector<DeviceInfo> devices;
    int selectedDevice = -1;
    bool scanning = false;
    QString scanError;

    // Operation ---------------------------------------------------------------
    bool busy = false;
    QString busyLabel;
    QJsonObject result;
    bool hasResult = false;
    QString prettyJson;
    QString lastOp;
    bool lastSuccess = false;
    QString lastErrorKind;
    QString lastErrorMessage;

    // Operation parameters ----------------------------------------------------
    QString writelkValue = QStringLiteral("80e48ed2");
    bool writelkControl = true;
    bool maintControl = true;
    QString probeKind = QStringLiteral("list");
    QString probeTimeout = QStringLiteral("1.0");
    QString batchProfile = QStringLiteral("basic");

    // Console -----------------------------------------------------------------
    QVector<LogLine> logs;
    bool autoScrollLogs = true;
    QElapsedTimer elapsed;

    AppState() { elapsed.start(); }

    void appendLog(const QString &text);
};

/// Result of validating the device form and building an FFI request.
struct RequestResult {
    bool ok = false;
    QJsonObject request;
    QString error;
};

/// Validate the device form, then embed `task` in a ready-to-send request.
RequestResult buildRequest(const AppState &state, const QJsonObject &task);

/// Parse `"0482"` / `"0x0482"` style hex input.
bool parseHexU16(const QString &text, quint16 *out, QString *error);
bool parseHexU8(const QString &text, quint8 *out, QString *error);
