use auroku::DeviceSummary;

#[derive(Clone)]
pub struct GrantedDevice {
    pub device: DeviceSummary,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub acl_state: String,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct AclSnapshot {
    user: Option<String>,
    mask: Option<String>,
}

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
const PKEXEC_CANDIDATES: &[&str] = &["/usr/bin/pkexec", "/bin/pkexec"];

#[cfg(target_os = "linux")]
const SETFACL_CANDIDATES: &[&str] = &["/usr/bin/setfacl", "/bin/setfacl"];

#[cfg(target_os = "linux")]
const GETFACL_CANDIDATES: &[&str] = &["/usr/bin/getfacl", "/bin/getfacl"];

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct DeviceAddress {
    vid: u16,
    pid: u16,
    bus: u8,
    address: u8,
}

#[cfg(target_os = "linux")]
impl From<&DeviceSummary> for DeviceAddress {
    fn from(device: &DeviceSummary) -> Self {
        Self {
            vid: device.vid,
            pid: device.pid,
            bus: device.bus,
            address: device.address,
        }
    }
}

#[cfg(target_os = "linux")]
pub fn refuse_root_execution() -> bool {
    if rustix::process::geteuid().is_root() {
        eprintln!(
            "[auroku-gui] {}",
            crate::i18n::t("GUI 必须以普通用户运行；请选择设备后再授权 USB 访问。")
        );
        true
    } else {
        false
    }
}

#[cfg(not(target_os = "linux"))]
pub fn refuse_root_execution() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub fn grant_selected_device(device: &DeviceSummary) -> Result<String, String> {
    let uid = rustix::process::geteuid().as_raw();
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let executable = trusted_executable(&executable, Some(uid))
        .ok_or_else(|| "GUI executable path has unsafe permissions".to_owned())?;
    let pkexec = find_root_executable(PKEXEC_CANDIDATES)
        .ok_or_else(|| "pkexec was not found in a trusted system directory".to_owned())?;

    let output = Command::new(pkexec)
        .arg(executable)
        .arg("--grant-usb-access")
        .arg(device.vid.to_string())
        .arg(device.pid.to_string())
        .arg(device.bus.to_string())
        .arg(device.address.to_string())
        .output()
        .map_err(|error| format!("failed to start polkit: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "polkit USB permission helper failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    parse_acl_state(&String::from_utf8_lossy(&output.stdout))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(not(target_os = "linux"))]
pub fn grant_selected_device(_device: &DeviceSummary) -> Result<String, String> {
    Ok(String::new())
}

#[cfg(target_os = "linux")]
pub fn revoke_granted_devices(devices: &[GrantedDevice]) -> Result<(), String> {
    if devices.is_empty() {
        return Ok(());
    }

    let uid = rustix::process::geteuid().as_raw();
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let executable = trusted_executable(&executable, Some(uid))
        .ok_or_else(|| "GUI executable path has unsafe permissions".to_owned())?;
    let pkexec = find_root_executable(PKEXEC_CANDIDATES)
        .ok_or_else(|| "pkexec was not found in a trusted system directory".to_owned())?;

    let mut command = Command::new(pkexec);
    command.arg(executable).arg("--revoke-usb-access");
    for grant in devices {
        command
            .arg(grant.device.vid.to_string())
            .arg(grant.device.pid.to_string())
            .arg(grant.device.bus.to_string())
            .arg(grant.device.address.to_string())
            .arg(&grant.acl_state);
    }

    let output = command
        .output()
        .map_err(|error| format!("failed to start polkit for USB ACL cleanup: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "polkit USB ACL cleanup failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn revoke_granted_devices(_devices: &[GrantedDevice]) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn run_privileged_helper() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next()?.into_string().ok()?;
    if mode != "--grant-usb-access" && mode != "--revoke-usb-access" {
        return None;
    }

    let result = (|| {
        let uid = std::env::var("PKEXEC_UID")
            .map_err(|_| "pkexec did not provide the requesting user ID".to_owned())?
            .parse::<u32>()
            .map_err(|_| "invalid requesting user ID".to_owned())?;
        if uid == 0 || !rustix::process::geteuid().is_root() {
            return Err(
                "USB permission helper must be run by pkexec for a non-root user".to_owned(),
            );
        }

        match mode.as_str() {
            "--grant-usb-access" => {
                let device = parse_device(&mut args)?;
                if args.next().is_some() {
                    return Err("unexpected helper arguments".to_owned());
                }
                let acl_state = grant_usb_node(&device, uid)?;
                println!("{acl_state}");
            }
            "--revoke-usb-access" => {
                let mut devices = Vec::new();
                while let Some(vid) = args.next() {
                    let device = DeviceAddress {
                        vid: parse_arg(Some(vid), "VID")?,
                        pid: parse_arg(args.next(), "PID")?,
                        bus: parse_arg(args.next(), "bus")?,
                        address: parse_arg(args.next(), "address")?,
                    };
                    let acl_state = parse_arg::<String>(args.next(), "ACL state")?;
                    devices.push((device, parse_acl_state(&acl_state)?));
                }
                if devices.is_empty() {
                    return Err("no USB devices were provided for ACL cleanup".to_owned());
                }
                let mut errors = Vec::new();
                for (device, acl_state) in devices {
                    if let Err(error) = revoke_usb_node(&device, uid, &acl_state) {
                        errors.push(error);
                    }
                }
                if !errors.is_empty() {
                    return Err(errors.join("; "));
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    })();

    match result {
        Ok(()) => Some(0),
        Err(error) => {
            eprintln!("[auroku-gui] USB permission helper failed: {error}");
            Some(1)
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn run_privileged_helper() -> Option<i32> {
    None
}

#[cfg(target_os = "linux")]
fn parse_device(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<DeviceAddress, String> {
    Ok(DeviceAddress {
        vid: parse_arg(args.next(), "VID")?,
        pid: parse_arg(args.next(), "PID")?,
        bus: parse_arg(args.next(), "bus")?,
        address: parse_arg(args.next(), "address")?,
    })
}

#[cfg(target_os = "linux")]
fn grant_usb_node(device: &DeviceAddress, uid: u32) -> Result<String, String> {
    let node = verified_usb_node(device)?;
    let previous_acl = read_acl_snapshot(&node, uid)?;
    set_user_acl(&node, uid, Some("rw-"))?;
    let granted_acl = read_acl_snapshot(&node, uid)?;
    Ok(encode_acl_state(&previous_acl, &granted_acl))
}

#[cfg(target_os = "linux")]
fn revoke_usb_node(
    device: &DeviceAddress,
    uid: u32,
    acl_state: &(AclSnapshot, AclSnapshot),
) -> Result<(), String> {
    let devices = auroku::scan_devices(Some(device.vid), Some(device.pid))
        .map_err(|error| error.to_string())?;
    if !devices
        .iter()
        .any(|found| found.bus == device.bus && found.address == device.address)
    {
        return Ok(());
    }

    let node = verified_usb_node(device)?;
    let current_acl = read_acl_snapshot(&node, uid)?;
    let (previous_acl, granted_acl) = acl_state;
    if &current_acl != granted_acl {
        return if &current_acl == previous_acl {
            Ok(())
        } else {
            Err("USB node ACL changed after authorization; leaving it untouched".to_owned())
        };
    }
    restore_acl_snapshot(&node, uid, previous_acl)
}

#[cfg(target_os = "linux")]
fn verified_usb_node(device: &DeviceAddress) -> Result<PathBuf, String> {
    use std::os::unix::fs::FileTypeExt;

    let devices = auroku::scan_devices(Some(device.vid), Some(device.pid))
        .map_err(|error| error.to_string())?;
    if !devices
        .iter()
        .any(|found| found.bus == device.bus && found.address == device.address)
    {
        return Err("selected USB device is no longer present".to_owned());
    }

    let node = PathBuf::from(format!(
        "/dev/bus/usb/{:03}/{:03}",
        device.bus, device.address
    ));
    let canonical = std::fs::canonicalize(&node).map_err(|error| error.to_string())?;
    if !canonical.starts_with("/dev/bus/usb")
        || !std::fs::metadata(&canonical)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_char_device()
    {
        return Err("selected USB node is invalid".to_owned());
    }
    Ok(canonical)
}

#[cfg(target_os = "linux")]
fn read_acl_snapshot(node: &Path, uid: u32) -> Result<AclSnapshot, String> {
    let getfacl = find_root_executable(GETFACL_CANDIDATES)
        .ok_or_else(|| "getfacl was not found in a trusted system directory".to_owned())?;
    let output = Command::new(getfacl)
        .arg("-cpn")
        .arg("--")
        .arg(node)
        .output()
        .map_err(|error| format!("failed to read USB node ACL: {error}"))?;
    if !output.status.success() {
        return Err(format!("getfacl exited with {}", output.status));
    }

    let user_prefix = format!("user:{uid}:");
    let mut snapshot = AclSnapshot {
        user: None,
        mask: None,
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let entry = line.split('#').next().unwrap_or_default().trim();
        if let Some(permissions) = entry.strip_prefix(&user_prefix) {
            snapshot.user = Some(validate_acl_permissions(permissions)?);
        } else if let Some(permissions) = entry.strip_prefix("mask::") {
            snapshot.mask = Some(validate_acl_permissions(permissions)?);
        }
    }
    Ok(snapshot)
}

#[cfg(target_os = "linux")]
fn set_user_acl(node: &Path, uid: u32, permissions: Option<&str>) -> Result<(), String> {
    match permissions {
        Some(permissions) => set_acl_command(node, "-m", &format!("u:{uid}:{permissions}")),
        None => set_acl_command(node, "-x", &format!("u:{uid}")),
    }
}

#[cfg(target_os = "linux")]
fn restore_acl_snapshot(node: &Path, uid: u32, previous: &AclSnapshot) -> Result<(), String> {
    set_user_acl(node, uid, previous.user.as_deref())?;
    if let Some(mask) = previous.mask.as_deref() {
        set_mask_acl(node, mask)?;
    } else if read_acl_snapshot(node, uid)?.mask.is_some() {
        remove_mask_acl(node)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_mask_acl(node: &Path, permissions: &str) -> Result<(), String> {
    set_acl_command(node, "-m", &format!("m:{permissions}"))
}

#[cfg(target_os = "linux")]
fn remove_mask_acl(node: &Path) -> Result<(), String> {
    set_acl_command(node, "-x", "m")
}

#[cfg(target_os = "linux")]
fn set_acl_command(node: &Path, operation: &str, entry: &str) -> Result<(), String> {
    let setfacl = find_root_executable(SETFACL_CANDIDATES)
        .ok_or_else(|| "setfacl was not found in a trusted system directory".to_owned())?;
    let status = Command::new(setfacl)
        .arg(operation)
        .arg(entry)
        .arg("--")
        .arg(node)
        .status()
        .map_err(|error| format!("failed to update USB node ACL: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("setfacl exited with {status}"))
    }
}

#[cfg(target_os = "linux")]
fn encode_acl_state(previous: &AclSnapshot, granted: &AclSnapshot) -> String {
    format!(
        "{}|{}",
        encode_acl_snapshot(previous),
        encode_acl_snapshot(granted)
    )
}

#[cfg(target_os = "linux")]
fn encode_acl_snapshot(snapshot: &AclSnapshot) -> String {
    format!(
        "{},{}",
        snapshot.user.as_deref().unwrap_or("-"),
        snapshot.mask.as_deref().unwrap_or("-")
    )
}

#[cfg(target_os = "linux")]
fn parse_acl_state(value: &str) -> Result<(AclSnapshot, AclSnapshot), String> {
    let (previous, granted) = value
        .trim()
        .split_once('|')
        .ok_or_else(|| "invalid ACL state".to_owned())?;
    Ok((parse_acl_snapshot(previous)?, parse_acl_snapshot(granted)?))
}

#[cfg(target_os = "linux")]
fn parse_acl_snapshot(value: &str) -> Result<AclSnapshot, String> {
    let mut fields = value.split(',');
    let user = parse_optional_acl(fields.next())?;
    let mask = parse_optional_acl(fields.next())?;
    if fields.next().is_some() {
        return Err("invalid ACL snapshot".to_owned());
    }
    Ok(AclSnapshot { user, mask })
}

#[cfg(target_os = "linux")]
fn parse_optional_acl(value: Option<&str>) -> Result<Option<String>, String> {
    match value {
        Some("-") => Ok(None),
        Some(permissions) => validate_acl_permissions(permissions).map(Some),
        None => Err("invalid ACL snapshot".to_owned()),
    }
}

#[cfg(target_os = "linux")]
fn validate_acl_permissions(permissions: &str) -> Result<String, String> {
    if permissions.len() == 3
        && permissions
            .bytes()
            .zip(*b"rwx")
            .all(|(value, expected)| value == expected || value == b'-')
    {
        Ok(permissions.to_owned())
    } else {
        Err("invalid USB ACL permission entry".to_owned())
    }
}

#[cfg(target_os = "linux")]
fn parse_arg<T: std::str::FromStr>(
    value: Option<std::ffi::OsString>,
    name: &str,
) -> Result<T, String> {
    value
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("missing or invalid {name}"))
}

#[cfg(target_os = "linux")]
fn find_root_executable(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .find_map(|candidate| trusted_executable(Path::new(candidate), Some(0)))
}

#[cfg(target_os = "linux")]
fn trusted_executable(path: &Path, owner: Option<u32>) -> Option<PathBuf> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let canonical = std::fs::canonicalize(path).ok()?;
    let metadata = std::fs::metadata(&canonical).ok()?;
    let mode = metadata.permissions().mode();
    if !metadata.is_file()
        || !owner.is_some_and(|uid| metadata.uid() == uid)
        || mode & 0o111 == 0
        || mode & 0o022 != 0
    {
        return None;
    }

    for ancestor in canonical.ancestors().skip(1) {
        let metadata = std::fs::metadata(ancestor).ok()?;
        if !metadata.is_dir()
            || !owner.is_some_and(|uid| metadata.uid() == uid || metadata.uid() == 0)
            || metadata.permissions().mode() & 0o022 != 0
        {
            return None;
        }
    }

    Some(canonical)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{parse_acl_state, parse_arg, AclSnapshot};
    use std::ffi::OsString;

    #[test]
    fn helper_arguments_are_strictly_parsed() {
        assert_eq!(parse_arg::<u8>(Some(OsString::from("12")), "bus"), Ok(12));
        assert!(parse_arg::<u8>(Some(OsString::from("999")), "bus").is_err());
        assert!(parse_arg::<u8>(None, "bus").is_err());
        assert_eq!(
            parse_acl_state("r--,r-x|rw-,rwx"),
            Ok((
                AclSnapshot {
                    user: Some("r--".to_owned()),
                    mask: Some("r-x".to_owned()),
                },
                AclSnapshot {
                    user: Some("rw-".to_owned()),
                    mask: Some("rwx".to_owned()),
                },
            ))
        );
        assert!(parse_acl_state("rwx,other").is_err());
    }
}
