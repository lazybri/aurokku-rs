//! Minimal static bindings for libwdi (WinUSB driver installation).
//!
//! This is the only crate in the workspace that allows unsafe code; the main
//! crate keeps its forbid(unsafe_code) guarantee. The declarations mirror
//! libwdi 1.5.x (libwdi.h) and are only built on Windows.

#![cfg(windows)]
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
const WDI_WINUSB: i32 = 0;
const INF_NAME: &str = "usb_device.inf";
const WORK_DIR_NAME: &str = "auroku-wdi";

/// Operation completed successfully.
pub const WDI_SUCCESS: c_int = 0;
/// Input/output error.
pub const WDI_ERROR_IO: c_int = -1;
/// Invalid parameter.
pub const WDI_ERROR_INVALID_PARAM: c_int = -2;
/// The calling process is not elevated.
pub const WDI_ERROR_NEEDS_ADMIN: c_int = -15;
/// Verbose logging level.
pub const WDI_LOG_LEVEL_INFO: c_int = 1;
/// Disable libwdi logging.
pub const WDI_LOG_LEVEL_NONE: c_int = 4;

/// C struct wdi_device_info (libwdi.h).
#[repr(C)]
pub struct WdiDeviceInfo {
    pub next: *mut WdiDeviceInfo,
    pub vid: u16,
    pub pid: u16,
    pub is_composite: i32,
    pub mi: u8,
    pub desc: *mut c_char,
    pub driver: *mut c_char,
    pub device_id: *mut c_char,
    pub hardware_id: *mut c_char,
    pub compatible_id: *mut c_char,
    pub upper_filter: *mut c_char,
    pub driver_version: u64,
}

/// C struct wdi_options_install_driver (libwdi.h).
#[repr(C)]
pub struct WdiOptionsInstallDriver {
    pub hwnd: *mut c_void,
    pub install_filter_driver: i32,
    pub pending_install_timeout: u32,
}

/// C struct wdi_options_prepare_driver (libwdi.h).
#[repr(C)]
pub struct WdiOptionsPrepareDriver {
    /// wdi_driver_type; 0 = WDI_WINUSB.
    pub driver_type: i32,
    pub vendor_name: *mut c_char,
    pub device_guid: *mut c_char,
    pub disable_cat: i32,
    pub disable_signing: i32,
    pub cert_subject: *mut c_char,
    pub use_wcid_driver: i32,
    pub external_inf: i32,
}

#[repr(C)]
struct TokenElevation {
    token_is_elevated: u32,
}

extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn OpenProcessToken(process: *mut c_void, desired_access: u32, token: *mut *mut c_void) -> i32;
    fn GetTokenInformation(
        token: *mut c_void,
        class: u32,
        info: *mut c_void,
        info_len: u32,
        return_len: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

extern "system" {
    fn wdi_prepare_driver(
        device_info: *mut WdiDeviceInfo,
        path: *const c_char,
        inf_name: *const c_char,
        options: *mut WdiOptionsPrepareDriver,
    ) -> c_int;

    fn wdi_install_driver(
        device_info: *mut WdiDeviceInfo,
        path: *const c_char,
        inf_name: *const c_char,
        options: *mut WdiOptionsInstallDriver,
    ) -> c_int;

    fn wdi_strerror(errcode: c_int) -> *const c_char;

    fn wdi_set_log_level(level: c_int) -> c_int;
}

/// Error returned by libwdi.
#[derive(Clone, Debug)]
pub struct WdiError {
    /// libwdi error code (see WDI_ERROR_*).
    pub code: c_int,
    /// Human readable description from wdi_strerror.
    pub message: String,
}

impl std::fmt::Display for WdiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (libwdi error {})", self.message, self.code)
    }
}

impl std::error::Error for WdiError {}

/// Whether the current process runs with an elevated token.
pub fn is_elevated() -> bool {
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION: u32 = 20;

    let mut token: *mut c_void = std::ptr::null_mut();
    let mut elevation = TokenElevation {
        token_is_elevated: 0,
    };
    let mut length = 0u32;

    // SAFETY: the handles are owned by this process and closed before return.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut elevation as *mut TokenElevation as *mut c_void,
            std::mem::size_of::<TokenElevation>() as u32,
            &mut length,
        );
        let _ = CloseHandle(token);
        ok != 0 && elevation.token_is_elevated != 0
    }
}

/// Set the libwdi log level (WDI_LOG_LEVEL_*).
pub fn set_log_level(level: c_int) {
    // Ignoring the return value: logging is best effort.
    unsafe {
        let _ = wdi_set_log_level(level);
    }
}

/// Install the WinUSB driver for the given device.
///
/// The calling process must be elevated; otherwise the call fails with
/// WDI_ERROR_NEEDS_ADMIN and the caller is expected to relaunch elevated.
pub fn install_winusb(vid: u16, pid: u16) -> Result<(), WdiError> {
    let description = CString::new("auRokku USB device").expect("static description");
    let vendor = CString::new("auRokku").expect("static vendor");
    let inf_name = CString::new(INF_NAME).expect("static inf name");

    // libwdi writes the generated INF/CAT here and rejects a NULL path.
    let work_dir = std::env::temp_dir().join(WORK_DIR_NAME);
    std::fs::create_dir_all(&work_dir).map_err(|error| WdiError {
        code: WDI_ERROR_IO,
        message: format!("cannot create {}: {error}", work_dir.display()),
    })?;
    let path = CString::new(work_dir.to_string_lossy().to_string()).map_err(|error| WdiError {
        code: WDI_ERROR_INVALID_PARAM,
        message: format!("invalid temporary path: {error}"),
    })?;

    let mut device = WdiDeviceInfo {
        next: std::ptr::null_mut(),
        vid,
        pid,
        is_composite: 0,
        mi: 0,
        desc: description.as_ptr() as *mut c_char,
        driver: std::ptr::null_mut(),
        device_id: std::ptr::null_mut(),
        hardware_id: std::ptr::null_mut(),
        compatible_id: std::ptr::null_mut(),
        upper_filter: std::ptr::null_mut(),
        driver_version: 0,
    };

    let mut prepare = WdiOptionsPrepareDriver {
        driver_type: WDI_WINUSB,
        vendor_name: vendor.as_ptr() as *mut c_char,
        device_guid: std::ptr::null_mut(),
        disable_cat: 0,
        disable_signing: 0,
        cert_subject: std::ptr::null_mut(),
        use_wcid_driver: 0,
        external_inf: 0,
    };

    // Generate the INF/CAT for this VID/PID first; wdi_install_driver needs the
    // directory that holds them.
    let code =
        unsafe { wdi_prepare_driver(&mut device, path.as_ptr(), inf_name.as_ptr(), &mut prepare) };
    if code != WDI_SUCCESS {
        return Err(WdiError {
            code,
            message: error_message(code),
        });
    }

    let mut options = WdiOptionsInstallDriver {
        hwnd: std::ptr::null_mut(),
        install_filter_driver: 0,
        pending_install_timeout: 60_000,
    };

    let code =
        unsafe { wdi_install_driver(&mut device, path.as_ptr(), inf_name.as_ptr(), &mut options) };
    if code == WDI_SUCCESS {
        return Ok(());
    }

    Err(WdiError {
        code,
        message: error_message(code),
    })
}

fn error_message(code: c_int) -> String {
    let pointer = unsafe { wdi_strerror(code) };
    if pointer.is_null() {
        return format!("libwdi error {code}");
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}
