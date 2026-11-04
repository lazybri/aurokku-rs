//! WinUSB driver provisioning for Windows.
//!
//! The supported devices are CDC composite devices: Windows binds
//! usbser.sys to the whole device, so libusb can never open it until a WinUSB
//! driver is installed for that device. This module wraps the small
//! installation flow around libwdi's wdi-simple.exe:
//!
//! * locate the helper (program directory, tools/, PATH, AUROKU_WDI_SIMPLE),
//! * run it elevated (UAC) and wait for the device to re-enumerate,
//! * record what was installed in driver-state.json so a session that ends
//!   unexpectedly can be recovered on the next start.
//!
//! Nothing here is automatic: the CLI and the GUI call install() only after the
//! user explicitly asks for it.

use std::fs;
use std::path::PathBuf;
#[cfg(windows)]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// File name of the libwdi command line helper.
pub const WDI_SIMPLE: &str = "wdi-simple.exe";
/// Environment variable that overrides the helper location.
pub const WDI_ENV: &str = "AUROKU_WDI_SIMPLE";
/// WinUSB device type passed to wdi-simple via --type.
#[cfg(all(windows, not(feature = "static-libwdi")))]
const WDI_TYPE_WINUSB: &str = "0";

/// What a previous session installed, kept for crash recovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverRecord {
    pub vid: u16,
    pub pid: u16,
    /// Unix timestamp of the installation.
    pub installed_at: u64,
    /// Whether the user asked to restore the original driver on exit.
    pub restore_on_exit: bool,
    /// Set once the original driver has been restored.
    pub restored: bool,
}

impl DriverRecord {
    fn to_json(&self) -> Value {
        json!({
            "vid": self.vid,
            "pid": self.pid,
            "installed_at": self.installed_at,
            "restore_on_exit": self.restore_on_exit,
            "restored": self.restored,
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            vid: u16::try_from(value.get("vid")?.as_u64()?).ok()?,
            pid: u16::try_from(value.get("pid")?.as_u64()?).ok()?,
            installed_at: value.get("installed_at")?.as_u64()?,
            restore_on_exit: value.get("restore_on_exit")?.as_bool()?,
            restored: value.get("restored")?.as_bool()?,
        })
    }
}

#[cfg(windows)]
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Directory that holds the driver state file.
pub fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("AUROKU_STATE_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    {
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(base).join("auroku");
        }
        if let Some(base) = std::env::var_os("APPDATA") {
            return PathBuf::from(base).join("auroku");
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = std::env::var_os("XDG_STATE_HOME") {
            return PathBuf::from(base).join("auroku");
        }
        if let Some(base) = std::env::var_os("HOME") {
            return PathBuf::from(base).join(".local/state/auroku");
        }
    }
    std::env::temp_dir().join("auroku")
}

/// Path of the driver state file.
pub fn state_path() -> PathBuf {
    state_dir().join("driver-state.json")
}

/// Load the recorded installation, if any.
pub fn load_record() -> Option<DriverRecord> {
    let text = fs::read_to_string(state_path()).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    DriverRecord::from_json(&value)
}

/// Persist the record, creating the state directory when needed.
pub fn save_record(record: &DriverRecord) -> Result<(), String> {
    save_record_at(&state_path(), record)
}

fn save_record_at(path: &std::path::Path, record: &DriverRecord) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&record.to_json())
        .map_err(|error| format!("cannot encode driver state: {error}"))?;
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Drop the record entirely (used after a successful restore).
pub fn clear_record() {
    let _ = fs::remove_file(state_path());
}

/// A previous session installed WinUSB but never restored the original driver.
///
/// This is the crash-recovery signal: the device may still be bound to WinUSB
/// even though the user expected the original behaviour back.
pub fn pending_restore() -> Option<DriverRecord> {
    load_record().filter(|record| record.restore_on_exit && !record.restored)
}

/// Locate wdi-simple.exe.
///
/// Search order: AUROKU_WDI_SIMPLE, the program directory, tools/ next to it,
/// the current directory's tools/, then PATH.
pub fn locate_wdi() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(WDI_ENV) {
        let path = PathBuf::from(value);
        return path.is_file().then_some(path);
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from));

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = &exe_dir {
        candidates.push(dir.join(WDI_SIMPLE));
        candidates.push(dir.join("tools").join(WDI_SIMPLE));
    }
    candidates.push(PathBuf::from("tools").join(WDI_SIMPLE));

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(WDI_SIMPLE);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Current function driver service for the device (usbser, WINUSB, ...).
///
/// Returns None when the registry cannot be queried.
#[cfg(windows)]
pub fn driver_service(vid: u16, pid: u16) -> Option<String> {
    use std::os::windows::process::CommandExt;

    let key = format!("HKLM\\SYSTEM\\CurrentControlSet\\Enum\\USB\\VID_{vid:04X}&PID_{pid:04X}");
    let output = std::process::Command::new("reg")
        .args(["query", &key, "/s", "/v", "Service"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Service") else {
            continue;
        };
        let Some(value) = rest.trim().strip_prefix("REG_SZ") else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

/// Current function driver service for the device (non-Windows stub).
#[cfg(not(windows))]
pub fn driver_service(_vid: u16, _pid: u16) -> Option<String> {
    None
}

#[cfg(all(windows, not(feature = "static-libwdi")))]
fn missing_wdi_message() -> String {
    format!(
        "cannot find {WDI_SIMPLE}. Put it next to the executable or in a tools/ \
         sub-directory, or set {WDI_ENV} to its full path."
    )
}

/// Receives human readable driver progress lines (GUI console, CLI stderr).
pub type DriverLog = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// Path of the driver operation log.
pub fn log_path() -> PathBuf {
    state_dir().join("driver.log")
}

/// UTC timestamp used as the log prefix.
#[cfg(windows)]
fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let day = seconds % 86_400;
    format!(
        "{:02}:{:02}:{:02}Z",
        day / 3600,
        (day % 3600) / 60,
        day % 60
    )
}

/// Append one line to the driver log; logging must never break an install.
#[cfg(windows)]
fn write_log_file(line: &str) {
    use std::io::Write;

    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(file, "[{}] {line}", timestamp());
}

/// Truncate the log file and start a new section for a user-triggered run.
#[cfg(windows)]
fn reset_log(header: &str) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &path,
        format!(
            "[{}] === {header} ===
",
            timestamp()
        ),
    );
}

/// Emit one progress line: always to the log file, then to the sink or stderr.
#[cfg(windows)]
fn emit(log: &Option<DriverLog>, line: impl AsRef<str>) {
    let text = line.as_ref();
    write_log_file(text);
    match log {
        Some(sink) => sink(text),
        None => eprintln!("[driver] {text}"),
    }
}
/// Install WinUSB for the device and wait until libusb can open it.
///
/// Requires an interactive UAC confirmation; the helper is started elevated.
#[cfg(windows)]
pub fn install(
    vid: u16,
    pid: u16,
    restore_on_exit: bool,
    log: Option<DriverLog>,
    append_log: bool,
) -> Result<DriverRecord, String> {
    if !append_log {
        reset_log(&format!("install {vid:04X}:{pid:04X}"));
    }
    emit(
        &log,
        format!("正在为 {vid:04X}:{pid:04X} 准备 WinUSB 驱动…"),
    );
    install_backend(vid, pid, log.clone())?;
    emit(&log, "等待设备切换到 WinUSB…");
    wait_until(true, vid, pid, Duration::from_secs(30), &log)?;
    emit(&log, "WinUSB 驱动已就绪");

    let record = DriverRecord {
        vid,
        pid,
        installed_at: now_unix(),
        restore_on_exit,
        restored: false,
    };
    save_record(&record)?;
    Ok(record)
}

/// Install WinUSB for the device (non-Windows stub).
#[cfg(not(windows))]
pub fn install(
    _vid: u16,
    _pid: u16,
    _restore_on_exit: bool,
    _log: Option<DriverLog>,
    _append_log: bool,
) -> Result<DriverRecord, String> {
    Err("WinUSB driver installation is only supported on Windows".to_owned())
}

/// Restore the original driver (usbser) and mark the record as restored.
#[cfg(windows)]
pub fn restore(vid: u16, pid: u16, log: Option<DriverLog>, append_log: bool) -> Result<(), String> {
    if !append_log {
        reset_log(&format!("restore {vid:04X}:{pid:04X}"));
    }
    emit(
        &log,
        format!("正在为 {vid:04X}:{pid:04X} 移除 WinUSB 驱动…"),
    );
    restore_backend(vid, pid, log.clone())?;
    emit(&log, "等待设备恢复为 usbser…");
    wait_until(false, vid, pid, Duration::from_secs(30), &log)?;
    emit(&log, "已恢复原始驱动");

    if let Some(mut record) = load_record().filter(|record| record.vid == vid && record.pid == pid)
    {
        record.restored = true;
        let _ = save_record(&record);
    }
    Ok(())
}

/// Restore the original driver (non-Windows stub).
#[cfg(not(windows))]
pub fn restore(
    _vid: u16,
    _pid: u16,
    _log: Option<DriverLog>,
    _append_log: bool,
) -> Result<(), String> {
    Err("WinUSB driver installation is only supported on Windows".to_owned())
}

// --------------------------------------------------------------------------
// usbser.sys service control (testing helper)
// --------------------------------------------------------------------------

/// Original `Start` value saved before disabling the usbser.sys service.
#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbSerRecord {
    pub start: u32,
    pub disabled_at: u64,
}

#[cfg(windows)]
impl UsbSerRecord {
    fn to_json(&self) -> Value {
        json!({
            "start": self.start,
            "disabled_at": self.disabled_at,
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            start: u32::try_from(value.get("start")?.as_u64()?).ok()?,
            disabled_at: value.get("disabled_at")?.as_u64()?,
        })
    }
}

#[cfg(windows)]
fn usbser_record_path() -> PathBuf {
    state_dir().join("usbser-state.json")
}

#[cfg(windows)]
fn load_usbser_record() -> Option<UsbSerRecord> {
    let text = fs::read_to_string(usbser_record_path()).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    UsbSerRecord::from_json(&value)
}

#[cfg(windows)]
fn save_usbser_record(record: &UsbSerRecord) -> Result<(), String> {
    let path = usbser_record_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&record.to_json())
        .map_err(|error| format!("cannot encode usbser state: {error}"))?;
    fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(windows)]
fn clear_usbser_record() {
    let _ = fs::remove_file(usbser_record_path());
}

/// Current `Start` value of the usbser.sys kernel service.
///
/// 2 = AUTO_START, 3 = DEMAND_START (Windows default), 4 = DISABLED.
#[cfg(windows)]
pub fn usbser_start() -> Option<u32> {
    use std::os::windows::process::CommandExt;

    let output = std::process::Command::new("reg")
        .args([
            "query",
            "HKLM\\SYSTEM\\CurrentControlSet\\Services\\usbser",
            "/v",
            "Start",
        ])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("Start") || parts.next() != Some("REG_DWORD") {
            continue;
        }
        let value = parts.next()?.trim_start_matches("0x");
        return u32::from_str_radix(value, 16).ok();
    }
    None
}

/// Non-Windows stub: the usbser service only exists on Windows.
#[cfg(not(windows))]
pub fn usbser_start() -> Option<u32> {
    None
}

#[cfg(windows)]
fn set_usbser_start(value: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let status = std::process::Command::new("reg")
        .args([
            "add",
            "HKLM\\SYSTEM\\CurrentControlSet\\Services\\usbser",
            "/v",
            "Start",
            "/t",
            "REG_DWORD",
            "/d",
            &value.to_string(),
            "/f",
        ])
        .creation_flags(0x0800_0000)
        .status()
        .map_err(|error| format!("cannot run reg.exe: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "reg.exe failed to set usbser Start={value} (elevation denied?)"
        ))
    }
}

/// Whether the current process has administrator rights.
#[cfg(windows)]
fn is_elevated() -> bool {
    use std::os::windows::process::CommandExt;

    static ELEVATED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ELEVATED.get_or_init(|| {
        let script = "([Security.Principal.WindowsPrincipal]\
                      [Security.Principal.WindowsIdentity]::GetCurrent())\
                      .IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .ok()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .eq_ignore_ascii_case("true")
            })
            .unwrap_or(false)
    })
}

/// Disable the usbser.sys kernel service (Start=4), saving its previous value.
///
/// Requires one UAC prompt when the GUI/CLI is not already elevated. A reboot or
/// a device re-enumeration is needed for the change to take effect.
#[cfg(windows)]
pub fn disable_usbser(log: Option<DriverLog>, append_log: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    if !append_log {
        reset_log("disable-usbser");
    }
    if !is_elevated() {
        emit(&log, "需要管理员权限，正在请求 UAC 确认…");
        return elevate_self(&driver_args("disable-usbser", 0, 0));
    }

    let current = usbser_start().ok_or_else(|| {
        "cannot read HKLM\\SYSTEM\\CurrentControlSet\\Services\\usbser".to_owned()
    })?;
    if current != 4 {
        if load_usbser_record().is_none() {
            save_usbser_record(&UsbSerRecord {
                start: current,
                disabled_at: now_unix(),
            })?;
        }
        set_usbser_start(4)?;
    }
    let _ = std::process::Command::new("sc")
        .args(["stop", "usbser"])
        .creation_flags(0x0800_0000)
        .output();

    emit(
        &log,
        "usbser.sys 已禁用（Start=4）；请重启 Windows 或重新插拔设备",
    );
    Ok(())
}

/// Disable the usbser.sys kernel service (non-Windows stub).
#[cfg(not(windows))]
pub fn disable_usbser(_log: Option<DriverLog>, _append_log: bool) -> Result<(), String> {
    Err("usbser.sys service control is only supported on Windows".to_owned())
}

/// Restore the saved usbser.sys Start value (default 3 if no backup exists).
#[cfg(windows)]
pub fn restore_usbser(log: Option<DriverLog>, append_log: bool) -> Result<(), String> {
    if !append_log {
        reset_log("restore-usbser");
    }
    if !is_elevated() {
        emit(&log, "需要管理员权限，正在请求 UAC 确认…");
        return elevate_self(&driver_args("restore-usbser", 0, 0));
    }

    let target = load_usbser_record().map(|record| record.start).unwrap_or(3);
    if target == 4 {
        return Err("saved usbser Start value is 4 (disabled); refusing to restore".to_owned());
    }
    set_usbser_start(target)?;
    clear_usbser_record();
    emit(
        &log,
        format!("usbser.sys 已恢复为 Start={target}；请重启 Windows 或重新插拔设备"),
    );
    Ok(())
}

/// Restore the usbser.sys Start value (non-Windows stub).
#[cfg(not(windows))]
pub fn restore_usbser(_log: Option<DriverLog>, _append_log: bool) -> Result<(), String> {
    Err("usbser.sys service control is only supported on Windows".to_owned())
}

// --------------------------------------------------------------------------
// Device installation policy: block usbser.sys before first install
// --------------------------------------------------------------------------

/// Ports device setup class GUID (usbser.inf belongs to this class).
#[cfg(windows)]
const PORTS_CLASS_GUID: &str = "{4d36e978-e325-11ce-bfc1-08002be10318}";
/// Device installation restrictions policy root.
#[cfg(windows)]
const DEVICE_INSTALL_POLICY_KEY: &str =
    "HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeviceInstall\\Restrictions";
/// DenyDeviceClasses subkey of the restrictions policy.
#[cfg(windows)]
const DENY_DEVICE_CLASSES_KEY: &str =
    "HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeviceInstall\\Restrictions\\DenyDeviceClasses";
/// Policy entry used by auRokku (only one class is denied).
#[cfg(windows)]
const DENY_DEVICE_CLASSES_ENTRY_KEY: &str =
    "HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeviceInstall\\Restrictions\\DenyDeviceClasses\\1";
/// Saved policy state, so Restore can undo exactly what was added.
#[cfg(windows)]
const POLICY_STATE_FILE: &str = "usbser-policy-state.json";
/// Export of the previous restrictions key (if it existed).
#[cfg(windows)]
const POLICY_BACKUP_FILE: &str = "device-install-policy-backup.reg";

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PolicyRecord {
    key_existed: bool,
    backup_file: bool,
    applied_at: u64,
}

#[cfg(windows)]
impl PolicyRecord {
    fn to_json(&self) -> Value {
        json!({
            "key_existed": self.key_existed,
            "backup_file": self.backup_file,
            "applied_at": self.applied_at,
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            key_existed: value.get("key_existed")?.as_bool()?,
            backup_file: value.get("backup_file")?.as_bool()?,
            applied_at: value.get("applied_at")?.as_u64()?,
        })
    }
}

#[cfg(windows)]
fn policy_state_path() -> PathBuf {
    state_dir().join(POLICY_STATE_FILE)
}

#[cfg(windows)]
fn policy_backup_path() -> PathBuf {
    state_dir().join(POLICY_BACKUP_FILE)
}

#[cfg(windows)]
fn load_policy_record() -> Option<PolicyRecord> {
    let text = fs::read_to_string(policy_state_path()).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    PolicyRecord::from_json(&value)
}

#[cfg(windows)]
fn save_policy_record(record: &PolicyRecord) -> Result<(), String> {
    let path = policy_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&record.to_json())
        .map_err(|error| format!("cannot encode policy state: {error}"))?;
    fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(windows)]
fn clear_policy_record() {
    let _ = fs::remove_file(policy_state_path());
}

#[cfg(windows)]
fn run_reg(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;

    std::process::Command::new("reg")
        .args(args)
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|error| format!("cannot run reg.exe: {error}"))
}

#[cfg(windows)]
fn reg_key_exists(key: &str) -> bool {
    run_reg(&["query", key])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn reg_query_string(key: &str, value_name: &str) -> Option<String> {
    let output = run_reg(&["query", key, "/v", value_name]).ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some(value_name) {
            continue;
        }
        if parts.next() != Some("REG_SZ") {
            return None;
        }
        let value = parts.collect::<Vec<_>>().join(" ");
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

#[cfg(windows)]
fn reg_query_dword(key: &str, value_name: &str) -> Option<u32> {
    let output = run_reg(&["query", key, "/v", value_name]).ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some(value_name) || parts.next() != Some("REG_DWORD") {
            continue;
        }
        let value = parts.next()?.trim_start_matches("0x");
        return u32::from_str_radix(value, 16).ok();
    }
    None
}

/// Decode a Windows registry export (.reg) as produced by reg.exe.
#[cfg(windows)]
fn decode_reg_export(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&units).ok();
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&units).ok();
    }
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Verify that a reg.exe export only touches the device-install Restrictions key.
/// The backup file lives in a user-writable directory, so an elevated restore
/// must not import unrelated HKLM keys from a tampered file.
#[cfg(windows)]
fn validate_policy_backup(path: &std::path::Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read policy backup: {error}"))?;
    let text = decode_reg_export(&bytes)
        .ok_or_else(|| "policy backup is not valid UTF-16/UTF-8 registry text".to_owned())?;
    let expected =
        "HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\DeviceInstall\\Restrictions";
    let expected_upper = expected.to_ascii_uppercase();
    let subkey_prefix = format!("{expected_upper}\\");
    let mut seen_key = false;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some(key) = rest.strip_suffix(']') else {
            continue;
        };
        let key_upper = key.to_ascii_uppercase();
        if key_upper != expected_upper && !key_upper.starts_with(&subkey_prefix) {
            return Err(format!(
                "policy backup contains an unexpected registry key: {key}"
            ));
        }
        seen_key = true;
    }
    if !seen_key {
        return Err("policy backup does not contain the Restrictions key".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn export_policy_backup(path: &std::path::Path) -> Result<bool, String> {
    if !reg_key_exists(DEVICE_INSTALL_POLICY_KEY) {
        return Ok(false);
    }
    let path_text = path.display().to_string();
    let output = run_reg(&["export", DEVICE_INSTALL_POLICY_KEY, &path_text, "/y"])?;
    if !output.status.success() {
        return Err(format!(
            "reg.exe export of the device-install policy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(true)
}

#[cfg(windows)]
fn import_policy_backup(path: &std::path::Path) -> Result<(), String> {
    let path_text = path.display().to_string();
    let output = run_reg(&["import", &path_text])?;
    if !output.status.success() {
        return Err(format!(
            "reg.exe import of the device-install policy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn refresh_device_install_policy() {
    use std::os::windows::process::CommandExt;

    // Device installation restrictions are read from the policy key; a policy
    // refresh makes the change visible sooner.
    let _ = std::process::Command::new("gpupdate")
        .args(["/force"])
        .creation_flags(0x0800_0000)
        .output();
}

/// Block new Ports-class driver installations, so usbser.inf cannot take over
/// a device on its first Windows connection.
///
/// This is a test-machine helper: while it is active, new COM/USB-serial
/// devices cannot install either. It does not affect devices that are already
/// installed. Undo with restore_usbser_policy.
#[cfg(windows)]
pub fn block_usbser_install(log: Option<DriverLog>, append_log: bool) -> Result<(), String> {
    if !append_log {
        reset_log("block-usbser-install");
    }
    if !is_elevated() {
        emit(&log, "需要管理员权限，正在请求 UAC 确认…");
        return elevate_self(&driver_args("block-usbser-install", 0, 0));
    }

    let entry_value = reg_query_string(DENY_DEVICE_CLASSES_ENTRY_KEY, "DeviceClass");
    if entry_value
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(PORTS_CLASS_GUID))
    {
        // The entry may be a leftover with the parent DWORD missing or 0. In
        // that case the restriction is NOT active; write the DWORD instead of
        // reporting a false success.
        if reg_query_dword(DEVICE_INSTALL_POLICY_KEY, "DenyDeviceClasses") == Some(1) {
            emit(&log, "Ports 类安装限制已处于启用状态");
            return Ok(());
        }
        if !policy_state_path().exists() {
            save_policy_record(&PolicyRecord {
                key_existed: false,
                backup_file: false,
                applied_at: now_unix(),
            })?;
        }
        let output = run_reg(&[
            "add",
            DEVICE_INSTALL_POLICY_KEY,
            "/v",
            "DenyDeviceClasses",
            "/t",
            "REG_DWORD",
            "/d",
            "1",
            "/f",
        ])?;
        if !output.status.success() {
            return Err(format!(
                "无法启用 DenyDeviceClasses：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        refresh_device_install_policy();
        emit(
            &log,
            "已补写 DenyDeviceClasses 策略值（Ports 类安装限制现在生效）",
        );
        return Ok(());
    }

    if let Some(existing) = entry_value {
        return Err(format!(
            "设备安装限制的 DenyDeviceClasses\\1 已被其它类占用（{existing}）；请先清理后再试"
        ));
    }

    if !policy_state_path().exists() {
        let _ = fs::remove_file(policy_backup_path());
        let had_backup = export_policy_backup(&policy_backup_path())?;
        save_policy_record(&PolicyRecord {
            key_existed: had_backup,
            backup_file: had_backup,
            applied_at: now_unix(),
        })?;
    }

    let output = run_reg(&[
        "add",
        DEVICE_INSTALL_POLICY_KEY,
        "/v",
        "DenyDeviceClasses",
        "/t",
        "REG_DWORD",
        "/d",
        "1",
        "/f",
    ])?;
    if !output.status.success() {
        return Err(format!(
            "无法启用 DenyDeviceClasses：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let output = run_reg(&[
        "add",
        DENY_DEVICE_CLASSES_ENTRY_KEY,
        "/v",
        "DeviceClass",
        "/t",
        "REG_SZ",
        "/d",
        PORTS_CLASS_GUID,
        "/f",
    ])?;
    if !output.status.success() {
        return Err(format!(
            "无法写入 Ports 类安装限制：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    refresh_device_install_policy();
    emit(
        &log,
        "已阻止 Ports 类驱动的新安装（usbser.inf）；插入设备后会停在无驱动状态，安装 WinUSB 后请点击恢复策略",
    );
    Ok(())
}

/// Block new Ports-class driver installations (non-Windows stub).
#[cfg(not(windows))]
pub fn block_usbser_install(_log: Option<DriverLog>, _append_log: bool) -> Result<(), String> {
    Err("device installation policy control is only supported on Windows".to_owned())
}

/// Restore the device installation policy saved by block_usbser_install.
#[cfg(windows)]
pub fn restore_usbser_policy(log: Option<DriverLog>, append_log: bool) -> Result<(), String> {
    if !append_log {
        reset_log("restore-usbser-policy");
    }
    if !is_elevated() {
        emit(&log, "需要管理员权限，正在请求 UAC 确认…");
        return elevate_self(&driver_args("restore-usbser-policy", 0, 0));
    }

    let Some(record) = load_policy_record() else {
        return Err("没有找到由 auRokku 保存的设备安装策略状态，未做修改".to_owned());
    };

    if record.backup_file {
        let backup = policy_backup_path();
        if !backup.is_file() {
            return Err(format!(
                "策略备份文件不存在（{}）；未做修改，以免丢失原有策略",
                backup.display()
            ));
        }
        validate_policy_backup(&backup)?;
    }

    // Always remove our explicit entry first. reg import() restores values but
    // does not delete entries that were not in the backup, so a stale backup
    // must not be the only cleanup step.
    let _ = run_reg(&["delete", DENY_DEVICE_CLASSES_ENTRY_KEY, "/f"]);
    let _ = run_reg(&[
        "delete",
        DEVICE_INSTALL_POLICY_KEY,
        "/v",
        "DenyDeviceClasses",
        "/f",
    ]);

    if record.backup_file {
        import_policy_backup(&policy_backup_path())?;
    } else if !record.key_existed {
        let _ = run_reg(&["delete", DENY_DEVICE_CLASSES_KEY, "/f"]);
    }

    if reg_query_string(DENY_DEVICE_CLASSES_ENTRY_KEY, "DeviceClass")
        .is_some_and(|value| value.eq_ignore_ascii_case(PORTS_CLASS_GUID))
    {
        return Err("恢复设备安装策略后仍检测到 Ports 类限制，请检查注册表".to_owned());
    }

    let _ = fs::remove_file(policy_backup_path());
    clear_policy_record();
    refresh_device_install_policy();
    emit(&log, "已恢复设备安装策略（Ports 类不再被阻止）");
    Ok(())
}

/// Restore the device installation policy (non-Windows stub).
#[cfg(not(windows))]
pub fn restore_usbser_policy(_log: Option<DriverLog>, _append_log: bool) -> Result<(), String> {
    Err("device installation policy control is only supported on Windows".to_owned())
}

#[cfg(all(windows, not(feature = "static-libwdi")))]
fn wdi_args(uninstall: bool, vid: u16, pid: u16) -> Vec<String> {
    let mut args = vec![
        "--vid".to_owned(),
        format!("0x{vid:04x}"),
        "--pid".to_owned(),
        format!("0x{pid:04x}"),
        "--type".to_owned(),
        WDI_TYPE_WINUSB.to_owned(),
        "--name".to_owned(),
        "auRokku".to_owned(),
    ];
    if uninstall {
        args.push("--uninstall".to_owned());
    }
    args
}

/// Install the WinUSB driver with the backend selected at compile time.
///
/// The static backend calls libwdi in-process and only asks for elevation when
/// libwdi reports WDI_ERROR_NEEDS_ADMIN, so an already elevated shell never
/// sees an extra prompt.
#[cfg(all(windows, feature = "static-libwdi"))]
fn install_backend(vid: u16, pid: u16, log: Option<DriverLog>) -> Result<(), String> {
    libwdi_sys::set_log_level(libwdi_sys::WDI_LOG_LEVEL_INFO);
    emit(&log, "正在生成驱动包（INF/CAT）…");
    match libwdi_sys::install_winusb(vid, pid) {
        Ok(()) => Ok(()),
        Err(error) if error.code == libwdi_sys::WDI_ERROR_NEEDS_ADMIN => {
            emit(&log, "需要管理员权限，正在请求 UAC 确认…");
            elevate_self(&driver_args("install-driver", vid, pid))
        }
        Err(error) => Err(format!("libwdi: {error}")),
    }
}

/// Install WinUSB by running the external wdi-simple helper (fallback).
#[cfg(all(windows, not(feature = "static-libwdi")))]
fn install_backend(vid: u16, pid: u16, _log: Option<DriverLog>) -> Result<(), String> {
    let wdi = locate_wdi().ok_or_else(missing_wdi_message)?;
    let args = wdi_args(false, vid, pid);
    let code = run_elevated(&wdi, &args)?;
    if code != 0 {
        return Err(format!(
            "wdi-simple exited with code {code} (elevation cancelled or installation failed)"
        ));
    }
    Ok(())
}

/// Restore the original driver without libwdi.
///
/// Removing the device node alone is not enough: the libwdi package stays in the
/// driver store and PnP simply re-binds it on the next scan. Delete every
/// package that targets this VID/PID first, then drop the node.
#[cfg(all(windows, feature = "static-libwdi"))]
fn restore_backend(vid: u16, pid: u16, log: Option<DriverLog>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    if !libwdi_sys::is_elevated() {
        emit(&log, "需要管理员权限，正在请求 UAC 确认…");
        return elevate_self(&driver_args("restore-driver", vid, pid));
    }

    let packages = driver_packages(vid, pid);
    emit(&log, format!("找到 {} 个驱动包待删除", packages.len()));
    for package in &packages {
        let _ = std::process::Command::new("pnputil")
            .args(["/delete-driver", package, "/uninstall", "/force"])
            .creation_flags(0x0800_0000)
            .status();
    }

    remove_device(vid, pid)
}

/// Restore the original driver through wdi-simple --uninstall (fallback).
#[cfg(all(windows, not(feature = "static-libwdi")))]
fn restore_backend(vid: u16, pid: u16, _log: Option<DriverLog>) -> Result<(), String> {
    let wdi = locate_wdi().ok_or_else(missing_wdi_message)?;
    let args = wdi_args(true, vid, pid);
    let code = run_elevated(&wdi, &args)?;
    if code != 0 {
        return Err(format!(
            "wdi-simple --uninstall exited with code {code} (elevation cancelled?)"
        ));
    }
    Ok(())
}

/// Command line for re-running ourselves elevated: auroku --op install-driver ...
#[cfg(windows)]
fn driver_args(operation: &str, vid: u16, pid: u16) -> Vec<String> {
    vec![
        "--op".to_owned(),
        operation.to_owned(),
        "--vid".to_owned(),
        format!("{vid:04x}"),
        "--pid".to_owned(),
        format!("{pid:04x}"),
        "--driver-log-append".to_owned(),
    ]
}

/// Relaunch the CLI elevated and wait for it: exactly one UAC prompt, no
/// PowerShell window, and the UAC dialog names auroku.exe.
#[cfg(windows)]
fn elevate_self(args: &[String]) -> Result<(), String> {
    let current =
        std::env::current_exe().map_err(|error| format!("cannot locate executable: {error}"))?;
    // The GUI must elevate the CLI binary: auroku-gui does not parse --op.
    let cli = current
        .parent()
        .map(|dir| dir.join("auroku.exe"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| current.clone());

    let code = run_elevated(&cli, args)?;
    if code == 0 {
        Ok(())
    } else {
        Err(format!(
            "the elevated helper exited with code {code}; the request was cancelled or failed"
        ))
    }
}

/// Remove the device node (if still present) and rescan the bus.
#[cfg(all(windows, feature = "static-libwdi"))]
fn remove_device(vid: u16, pid: u16) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    if let Some(instance) = device_instance_ids(vid, pid).into_iter().next() {
        let _ = std::process::Command::new("pnputil")
            .args(["/remove-device", &instance])
            .creation_flags(0x0800_0000)
            .status();
    }

    let _ = std::process::Command::new("pnputil")
        .arg("/scan-devices")
        .creation_flags(0x0800_0000)
        .status();
    Ok(())
}

/// Whether an INF file mentions the given ASCII marker.
///
/// INF files are usually ASCII but may be UTF-16LE (with or without BOM), so
/// look for both the narrow and the widened byte pattern.
#[cfg(all(windows, feature = "static-libwdi"))]
fn inf_mentions(bytes: &[u8], needle: &[u8]) -> bool {
    if bytes
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
    {
        return true;
    }

    let wide: Vec<u8> = needle.iter().flat_map(|byte| [*byte, 0]).collect();
    bytes
        .windows(wide.len())
        .any(|window| window.eq_ignore_ascii_case(&wide))
}

/// Published OEM packages (oemNN.inf) whose INF targets this VID/PID.
///
/// libwdi generates one package per install attempt, so multiple entries are
/// normal; all of them have to go for the original driver to win again.
#[cfg(all(windows, feature = "static-libwdi"))]
fn driver_packages(vid: u16, pid: u16) -> Vec<String> {
    let needle = format!("VID_{vid:04X}&PID_{pid:04X}").into_bytes();
    let mut packages = Vec::new();

    let Ok(entries) = std::fs::read_dir("C:\\Windows\\INF") else {
        return packages;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.starts_with("oem") || !name.ends_with(".inf") {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        if inf_mentions(&bytes, &needle) {
            packages.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    packages
}

/// Device instance IDs (USB\VID_....\serial) of the connected device.
#[cfg(all(windows, feature = "static-libwdi"))]
fn device_instance_ids(vid: u16, pid: u16) -> Vec<String> {
    use std::os::windows::process::CommandExt;

    let key = format!("HKLM\\SYSTEM\\CurrentControlSet\\Enum\\USB\\VID_{vid:04X}&PID_{pid:04X}");
    let Ok(output) = std::process::Command::new("reg")
        .args(["query", &key])
        .creation_flags(0x0800_0000)
        .output()
    else {
        return Vec::new();
    };

    let marker = format!("VID_{vid:04X}&PID_{pid:04X}\\");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let index = line.find(&marker)?;
            let instance = &line[index + marker.len()..];
            (!instance.is_empty() && !instance.contains('\\'))
                .then(|| format!("USB\\{marker}{instance}"))
        })
        .collect()
}
/// Run a helper through Start-Process -Verb RunAs so Windows raises UAC.
#[cfg(windows)]
fn run_elevated(exe: &std::path::Path, args: &[String]) -> Result<i32, String> {
    use std::os::windows::process::CommandExt;

    let quoted_exe = format!("'{}'", exe.display().to_string().replace('\'', "''"));
    let quoted_args = args
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        "$p = Start-Process -FilePath {quoted_exe} -ArgumentList {quoted_args} \
         -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode"
    );

    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(0x0800_0000)
        .status()
        .map_err(|error| format!("cannot start PowerShell: {error}"))?;

    Ok(status.code().unwrap_or(-1))
}

/// Poll until the device can (or can no longer) be opened by libusb.
#[cfg(windows)]
fn wait_until(
    openable: bool,
    vid: u16,
    pid: u16,
    timeout: Duration,
    _log: &Option<DriverLog>,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let service = driver_service(vid, pid);
        let ready = if openable {
            can_open(vid, pid)
        } else {
            // The node disappears while PnP re-enumerates; only a real service
            // that is no longer WINUSB counts as restored.
            matches!(service.as_deref(), Some(name) if !name.eq_ignore_ascii_case("WINUSB"))
        };
        if ready {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(if openable {
                format!(
                    "device {vid:04X}:{pid:04X} did not become accessible through WinUSB \
                     within {}s (current service: {})",
                    timeout.as_secs(),
                    service.as_deref().unwrap_or("unknown")
                )
            } else {
                format!(
                    "device {vid:04X}:{pid:04X} did not return to its original driver within {}s                      (current service: {})",
                    timeout.as_secs(),
                    service.as_deref().unwrap_or("not present")
                )
            });
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

#[cfg(windows)]
fn can_open(vid: u16, pid: u16) -> bool {
    use rusb::UsbContext;

    let Ok(context) = rusb::Context::new() else {
        return false;
    };
    let Ok(devices) = context.devices() else {
        return false;
    };
    for device in devices.iter() {
        let Ok(descriptor) = device.device_descriptor() else {
            continue;
        };
        if descriptor.vendor_id() == vid && descriptor.product_id() == pid {
            return device.open().is_ok();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_roundtrip() {
        let record = DriverRecord {
            vid: 0x04C5,
            pid: 0x1270,
            installed_at: 1_700_000_000,
            restore_on_exit: true,
            restored: false,
        };
        let encoded = record.to_json();
        assert_eq!(DriverRecord::from_json(&encoded), Some(record));
    }

    #[test]
    fn record_persists_to_disk() {
        let dir = std::env::current_dir()
            .expect("working directory")
            .join(format!(".auroku-test-{}", std::process::id()));
        let path = dir.join("driver-state.json");
        let record = DriverRecord {
            vid: 1,
            pid: 2,
            installed_at: 3,
            restore_on_exit: false,
            restored: true,
        };
        save_record_at(&path, &record).expect("save record");
        let text = fs::read_to_string(&path).expect("read record");
        let value: Value = serde_json::from_str(&text).expect("parse record");
        assert_eq!(DriverRecord::from_json(&value), Some(record));
        let _ = fs::remove_dir_all(&dir);
    }
}
