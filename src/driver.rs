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

/// Install WinUSB for the device and wait until libusb can open it.
///
/// Requires an interactive UAC confirmation; the helper is started elevated.
#[cfg(windows)]
pub fn install(vid: u16, pid: u16, restore_on_exit: bool) -> Result<DriverRecord, String> {
    install_backend(vid, pid)?;
    wait_until(true, vid, pid, Duration::from_secs(30))?;

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
pub fn install(_vid: u16, _pid: u16, _restore_on_exit: bool) -> Result<DriverRecord, String> {
    Err("WinUSB driver installation is only supported on Windows".to_owned())
}

/// Restore the original driver (usbser) and mark the record as restored.
#[cfg(windows)]
pub fn restore(vid: u16, pid: u16) -> Result<(), String> {
    restore_backend(vid, pid)?;
    wait_until(false, vid, pid, Duration::from_secs(20))?;

    if let Some(mut record) = load_record().filter(|record| record.vid == vid && record.pid == pid)
    {
        record.restored = true;
        let _ = save_record(&record);
    }
    Ok(())
}

/// Restore the original driver (non-Windows stub).
#[cfg(not(windows))]
pub fn restore(_vid: u16, _pid: u16) -> Result<(), String> {
    Err("WinUSB driver installation is only supported on Windows".to_owned())
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
fn install_backend(vid: u16, pid: u16) -> Result<(), String> {
    auroku_driver_sys::set_log_level(auroku_driver_sys::WDI_LOG_LEVEL_INFO);
    match auroku_driver_sys::install_winusb(vid, pid) {
        Ok(()) => Ok(()),
        Err(error) if error.code == auroku_driver_sys::WDI_ERROR_NEEDS_ADMIN => {
            elevate_self(&driver_args("install-driver", vid, pid))
        }
        Err(error) => Err(format!("libwdi: {error}")),
    }
}

/// Install WinUSB by running the external wdi-simple helper (fallback).
#[cfg(all(windows, not(feature = "static-libwdi")))]
fn install_backend(vid: u16, pid: u16) -> Result<(), String> {
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

/// Restore the original driver without libwdi: remove the device node so that
/// Windows re-enumerates it and rebinds usbser.
#[cfg(all(windows, feature = "static-libwdi"))]
fn restore_backend(vid: u16, pid: u16) -> Result<(), String> {
    if !auroku_driver_sys::is_elevated() {
        return elevate_self(&driver_args("restore-driver", vid, pid));
    }
    remove_device(vid, pid)
}

/// Restore the original driver through wdi-simple --uninstall (fallback).
#[cfg(all(windows, not(feature = "static-libwdi")))]
fn restore_backend(vid: u16, pid: u16) -> Result<(), String> {
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
#[cfg(all(windows, feature = "static-libwdi"))]
fn driver_args(operation: &str, vid: u16, pid: u16) -> Vec<String> {
    vec![
        "--op".to_owned(),
        operation.to_owned(),
        "--vid".to_owned(),
        format!("{vid:04x}"),
        "--pid".to_owned(),
        format!("{pid:04x}"),
    ]
}

/// Relaunch the CLI elevated and wait for it: exactly one UAC prompt, no
/// PowerShell window, and the UAC dialog names auroku.exe.
#[cfg(all(windows, feature = "static-libwdi"))]
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

/// Remove the device node; Windows re-enumerates it and rebinds usbser.
#[cfg(all(windows, feature = "static-libwdi"))]
fn remove_device(vid: u16, pid: u16) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let instance = device_instance_ids(vid, pid)
        .into_iter()
        .next()
        .ok_or_else(|| format!("device {vid:04X}:{pid:04X} is not present in the registry"))?;

    let status = std::process::Command::new("pnputil")
        .args(["/remove-device", &instance])
        .creation_flags(0x0800_0000)
        .status()
        .map_err(|error| format!("cannot run pnputil: {error}"))?;
    if !status.success() {
        return Err(format!(
            "pnputil /remove-device {instance} failed (code {:?})",
            status.code()
        ));
    }

    let _ = std::process::Command::new("pnputil")
        .arg("/scan-devices")
        .creation_flags(0x0800_0000)
        .status();
    Ok(())
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
         -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
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
fn wait_until(openable: bool, vid: u16, pid: u16, timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let service = driver_service(vid, pid);
        let ready = if openable {
            can_open(vid, pid)
        } else {
            service.as_deref() != Some("WINUSB")
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
                    "device {vid:04X}:{pid:04X} is still bound to WinUSB after {}s",
                    timeout.as_secs()
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
