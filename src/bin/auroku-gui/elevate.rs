//! Linux start-up privilege elevation via polkit / `pkexec`.
//!
//! auRokku talks to USB devices directly. On Linux this usually requires root
//! (or a matching udev rule). To match the CLI workflow, the GUI attempts a
//! polkit authentication at start-up and re-executes itself as root.
//!
//! Behaviour:
//! - Linux, non-root: `pkexec env <display/session vars> <exe> <args...>`.
//! - Authentication dismissed / polkit unavailable (`pkexec` exit 125/126/127):
//!   continue unprivileged so the UI can still be inspected; the GUI shows a
//!   permission warning.
//! - `pkexec` missing or unable to spawn: continue unprivileged.
//! - `--no-polkit` or `AUROKU_GUI_NO_POLKIT=1`: skip elevation explicitly.
//! - Already root, inside a `pkexec` child, or `AUROKU_GUI_ELEVATED=1`:
//!   never re-execute (loop guard).
//!
//! Only a small whitelist of environment variables is forwarded to the root
//! process so it can reach the user's display (Wayland or X11) and locale.

#[cfg(target_os = "linux")]
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::sync::OnceLock;

/// Environment variables forwarded into the elevated process.
#[cfg(target_os = "linux")]
const FORWARDED_ENV: &[&str] = &[
    // Display / session access
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP",
    "DISPLAY",
    "XAUTHORITY",
    "DBUS_SESSION_BUS_ADDRESS",
    // Escape hatch when a compositor rejects root Wayland clients:
    // WINIT_UNIX_BACKEND=x11 (requires XWayland) or wayland.
    "WINIT_UNIX_BACKEND",
    // Locale
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    // Application tuning
    "AUROKU_GUI_FONT",
    "RUST_LOG",
    "RUST_BACKTRACE",
];

/// Absolute locations commonly used for polkit's `pkexec`.
#[cfg(target_os = "linux")]
const PKEXEC_CANDIDATES: &[&str] = &[
    "/usr/bin/pkexec",
    "/bin/pkexec",
    "/usr/sbin/pkexec",
    "/run/current-system/sw/bin/pkexec", // NixOS
];

/// Absolute locations commonly used for `env`.
#[cfg(target_os = "linux")]
const ENV_CANDIDATES: &[&str] = &["/usr/bin/env", "/bin/env"];

/// True when the process is effectively root.
///
/// The GUI never drops privileges, so the effective UID is cached to avoid a
/// syscall on every frame (the footer queries it while rendering).
#[cfg(target_os = "linux")]
pub fn is_effective_root() -> bool {
    static EFFECTIVE_ROOT: OnceLock<bool> = OnceLock::new();
    *EFFECTIVE_ROOT.get_or_init(|| rustix::process::geteuid().is_root())
}

/// Non-Linux platforms never auto-elevate.
#[cfg(not(target_os = "linux"))]
pub fn is_effective_root() -> bool {
    true
}

/// Non-Linux platforms never auto-elevate.
#[cfg(not(target_os = "linux"))]
pub fn maybe_elevate() {}

/// Attempt to re-execute the GUI as root through polkit.
///
/// Returns normally when the process should continue unprivileged. Exits the
/// process when the elevated child ran (propagating its exit code) or when
/// pkexec reported a hard failure.
#[cfg(target_os = "linux")]
pub fn maybe_elevate() {
    if is_effective_root() || opted_out() || already_elevated() {
        return;
    }

    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!(
                "[auroku-gui] {}",
                crate::i18n::tf(
                    "无法定位可执行文件：{error}；将以普通用户运行",
                    &[("error", &error.to_string())],
                )
            );
            return;
        }
    };

    if !executable.is_file() {
        eprintln!(
            "[auroku-gui] {}",
            crate::i18n::t("可执行文件路径无效；将以普通用户运行")
        );
        return;
    }
    if is_world_writable(&executable) {
        eprintln!(
            "[auroku-gui] {}",
            crate::i18n::t("可执行文件对任意用户可写，拒绝提权；请修复文件权限后重试")
        );
        return;
    }

    let Some(pkexec) = find_program("pkexec", PKEXEC_CANDIDATES) else {
        eprintln!(
            "[auroku-gui] {}",
            crate::i18n::t("未找到 pkexec（polkit），将以普通用户运行")
        );
        return;
    };

    let mut command = Command::new(&pkexec);

    // `pkexec` sanitises the environment, so use `env` to restore the display
    // and session variables the elevated GUI needs.
    if let Some(env_program) = find_program("env", ENV_CANDIDATES) {
        command.arg(env_program);
        for pair in forwarded_env() {
            command.arg(pair);
        }
        command.arg(OsString::from("AUROKU_GUI_ELEVATED=1"));
    }

    command.arg(&executable);
    for arg in std::env::args_os().skip(1) {
        if arg.as_os_str() != OsStr::new("--no-polkit") {
            command.arg(arg);
        }
    }

    eprintln!(
        "[auroku-gui] {}",
        crate::i18n::t("正在通过 polkit 请求 root 权限…")
    );

    match command.status() {
        Ok(status) => match status.code() {
            // polkit-specific statuses: auth dismissed / not authorised /
            // daemon unavailable. Fall back to the unprivileged UI.
            Some(125..=127) => {
                eprintln!(
                    "[auroku-gui] {}",
                    crate::i18n::t("polkit 认证未完成，将以普通用户继续运行（USB 访问可能受限）")
                );
            }
            Some(code) => std::process::exit(code),
            None => std::process::exit(1),
        },
        Err(error) => {
            eprintln!(
                "[auroku-gui] {}",
                crate::i18n::tf(
                    "无法启动 pkexec：{error}；将以普通用户运行",
                    &[("error", &error.to_string())],
                )
            );
        }
    }
}

#[cfg(target_os = "linux")]
fn opted_out() -> bool {
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg.as_os_str() == OsStr::new("--no-polkit"))
    {
        return true;
    }

    match std::env::var_os("AUROKU_GUI_NO_POLKIT") {
        Some(value) => {
            let value = value.to_string_lossy();
            !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
        }
        None => false,
    }
}

#[cfg(target_os = "linux")]
fn already_elevated() -> bool {
    std::env::var_os("AUROKU_GUI_ELEVATED").is_some() || std::env::var_os("PKEXEC_UID").is_some()
}

/// Build `KEY=VALUE` pairs for the whitelisted environment variables.
#[cfg(target_os = "linux")]
fn forwarded_env() -> Vec<OsString> {
    let mut out = Vec::new();

    for (key, value) in std::env::vars_os() {
        let key_text = key.to_string_lossy();
        if FORWARDED_ENV.contains(&key_text.as_ref()) || key_text.starts_with("LC_") {
            let mut pair = OsString::from(&key);
            pair.push("=");
            pair.push(value);
            out.push(pair);
        }
    }

    // If X11 does not advertise XAUTHORITY explicitly, root's HOME would point
    // at /root. Restore the user's conventional auth file when it exists.
    if std::env::var_os("XAUTHORITY").is_none() {
        if let Some(home) = std::env::var_os("HOME") {
            let candidate = Path::new(&home).join(".Xauthority");
            if candidate.is_file() {
                let mut pair = OsString::from("XAUTHORITY=");
                pair.push(candidate.as_os_str());
                out.push(pair);
            }
        }
    }

    out
}

#[cfg(target_os = "linux")]
fn find_program(name: &str, absolute_candidates: &[&str]) -> Option<PathBuf> {
    for candidate in absolute_candidates {
        let path = Path::new(candidate);
        if is_executable_file(path) && !is_world_writable(path) {
            return Some(path.to_owned());
        }
    }

    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate) && !is_world_writable(candidate))
}

#[cfg(target_os = "linux")]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_world_writable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o002 != 0)
        .unwrap_or(true)
}
