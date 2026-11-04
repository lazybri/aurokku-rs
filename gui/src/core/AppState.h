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

/// WinUSB driver state reported by `auroku_driver_status` (Windows only).
struct DriverInfo {
    /// A status call has completed at least once.
    bool known = false;
    /// The platform supports WinUSB provisioning (Windows).
    bool supported = false;
    /// A previous session left the WinUSB driver installed without restoring it.
    bool pending = false;
    quint16 pendingVid = 0;
    quint16 pendingPid = 0;
    bool pendingRestoreOnExit = false;
    /// Current Windows service of the driver target, when known.
    QString service;
    QString logPath;
    QString stateDir;
    /// A driver operation is currently running.
    bool busy = false;
    /// `install`, `restore` or empty.
    QString busyOp;

    QString pendingTarget() const
    {
        if (!pending)
            return {};
        return QStringLiteral("%1:%2")
            .arg(pendingVid, 4, 16, QLatin1Char('0'))
            .arg(pendingPid, 4, 16, QLatin1Char('0'));
    }
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

    // WinUSB driver (Windows) -------------------------------------------------
    DriverInfo driver;
    bool restoreDriverOnExit = false;
    /// Device that failed with kind=DriverError and needs a WinUSB driver.
    bool hasDriverTarget = false;
    quint16 driverTargetVid = 0;
    quint16 driverTargetPid = 0;
    /// Last dispatched task, kept so a successful driver install can retry it.
    QJsonObject lastTaskRequest;
    QString lastTaskLabel;
    bool hasRetryTask = false;

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
