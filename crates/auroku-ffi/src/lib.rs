//! C ABI bridge between the Qt front-end and the `auroku` Rust backend.
//!
//! The Rust backend is a synchronous, panic-safe library. This crate exposes a
//! deliberately small, JSON-based surface so the C++ side never has to model
//! Rust enums or structs:
//!
//! - [`auroku_scan_devices`] — enumerate USB devices, returns a JSON array.
//! - [`auroku_run_task`] — run one protocol operation described by a JSON
//!   request, streams diagnostic lines through a C callback.
//! - [`auroku_ffi_version`] — bridge version string.
//! - [`auroku_string_free`] — release a string returned by this library.
//!
//! Every returned string is a JSON envelope:
//!
//! ```json
//! { "ok": true,  "data": ... }
//! { "ok": false, "kind": "ValueError", "message": "..." }
//! ```
//!
//! All strings are allocated by Rust (`CString::into_raw`) and must be freed
//! with [`auroku_string_free`]. Function pointers are never stored; the log
//! callback is only invoked during the call that received it.

#![allow(unsafe_code)]

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use auroku::{
    run_task, scan_devices, AuRokkuError, BatchProfile, DeviceConfig, LogSink, ProbeKind, Task,
};
use serde_json::{json, Map, Value};

/// Version of the C ABI surface. Bump when function signatures change.
pub const FFI_ABI_VERSION: &str = "1";

/// Receives one human-readable diagnostic line.
///
/// The pointer is only valid for the duration of the callback; implementations
/// must copy the string. The callback may be invoked from the thread that
/// called [`auroku_run_task`], i.e. never from a Rust-owned thread.
pub type AurokuLogCallback =
    Option<unsafe extern "C" fn(line: *const c_char, user_data: *mut c_void)>;

// ==============================================================================
// JSON envelope helpers
// ==============================================================================

struct ApiError {
    kind: &'static str,
    message: String,
}

impl fmt::Debug for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl From<AuRokkuError> for ApiError {
    fn from(error: AuRokkuError) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl ApiError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: "ValueError",
            message: message.into(),
        }
    }
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(text) => text.into_raw(),
        Err(_) => {
            // Only reachable if a message contains an interior NUL byte.
            let fallback = json!({
                "ok": false,
                "kind": "ValueError",
                "message": "response contained an interior NUL byte",
            })
            .to_string();
            CString::new(fallback)
                .expect("static JSON has no interior NUL")
                .into_raw()
        }
    }
}

fn encode(result: Result<Value, ApiError>) -> *mut c_char {
    let envelope = match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(error) => json!({
            "ok": false,
            "kind": error.kind,
            "message": error.message,
        }),
    };
    to_c_string(envelope.to_string())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_owned()
    }
}

/// Run a closure, converting panics into an `ok: false` envelope.
fn guarded<F>(body: F) -> *mut c_char
where
    F: FnOnce() -> Result<Value, ApiError>,
{
    let outcome = catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|payload| {
        Err(ApiError {
            kind: "Panic",
            message: panic_message(payload),
        })
    });
    encode(outcome)
}

// ==============================================================================
// Request parsing
// ==============================================================================

fn as_u8(value: &Value, field: &str) -> Result<u8, ApiError> {
    value
        .as_u64()
        .filter(|raw| *raw <= u8::MAX as u64)
        .map(|raw| raw as u8)
        .ok_or_else(|| ApiError::invalid(format!("`{field}` must be an integer 0..=255")))
}

fn as_u16(value: &Value, field: &str) -> Result<u16, ApiError> {
    value
        .as_u64()
        .filter(|raw| *raw <= u16::MAX as u64)
        .map(|raw| raw as u16)
        .ok_or_else(|| ApiError::invalid(format!("`{field}` must be an integer 0..=65535")))
}

fn optional_u8(request: &Map<String, Value>, field: &str) -> Result<Option<u8>, ApiError> {
    match request.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => as_u8(value, field).map(Some),
    }
}

fn optional_string(request: &Map<String, Value>, field: &str) -> Option<String> {
    request
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn flag(request: &Map<String, Value>, field: &str) -> bool {
    request.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn build_config(
    request: &Map<String, Value>,
    log_callback: AurokuLogCallback,
    user_data: *mut c_void,
) -> Result<DeviceConfig, ApiError> {
    let vid = request
        .get("vid")
        .ok_or_else(|| ApiError::invalid("missing `vid`"))
        .and_then(|value| as_u16(value, "vid"))?;
    let pid = request
        .get("pid")
        .ok_or_else(|| ApiError::invalid("missing `pid`"))
        .and_then(|value| as_u16(value, "pid"))?;

    let mut config = DeviceConfig::new(vid, pid);
    config.sn = optional_string(request, "sn");
    config.interface = optional_u8(request, "interface")?;
    if let Some(value) = request.get("ep_out").filter(|value| !value.is_null()) {
        config.ep_out = as_u8(value, "ep_out")?;
    }
    if let Some(value) = request.get("ep_in").filter(|value| !value.is_null()) {
        config.ep_in = as_u8(value, "ep_in")?;
    }
    config.verbose = flag(request, "verbose");
    config.type2 = flag(request, "type2");

    if let Some(callback) = log_callback {
        let bridge = LogBridge {
            callback,
            user_data,
        };
        config.log_sink = Some(bridge.into_sink());
    }

    Ok(config)
}

fn build_task(request: &Value) -> Result<Task, ApiError> {
    let task = request
        .as_object()
        .ok_or_else(|| ApiError::invalid("missing `task` object"))?;
    let kind = task
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::invalid("`task.kind` must be a string"))?;

    match kind {
        "status" => Ok(Task::Status),
        "readuimlk" => Ok(Task::ReadUimlk),
        "clearlk" => Ok(Task::Clearlk),
        "writelk" => {
            let value = task
                .get("value")
                .and_then(Value::as_str)
                .map(|text| text.trim().replace(' ', "").to_ascii_lowercase())
                .filter(|text| text.len() == 8 && text.chars().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| {
                    ApiError::invalid(
                        "`task.value` must be 8 hex characters (4 bytes), e.g. 80e48ed2",
                    )
                })?;
            let control = task
                .get("control")
                .map(|value| as_u8(value, "task.control"))
                .transpose()?
                .unwrap_or(1);
            Ok(Task::Writelk { value, control })
        }
        "readiccid" => Ok(Task::ReadIccid),
        "readpin" => Ok(Task::Readpin),
        "resetpin" => Ok(Task::Resetpin),
        "maintenance_bit" => {
            let control = task
                .get("control")
                .map(|value| as_u8(value, "task.control"))
                .transpose()?
                .unwrap_or(0);
            Ok(Task::MaintenanceBit { control })
        }
        "batch" => {
            let profile = task
                .get("profile")
                .and_then(Value::as_str)
                .and_then(BatchProfile::from_name)
                .ok_or_else(|| ApiError::invalid("`task.profile` must be `basic` or `uimlk`"))?;
            Ok(Task::Batch { profile })
        }
        "probe" => {
            let kind = task
                .get("probe")
                .and_then(Value::as_str)
                .and_then(ProbeKind::from_name)
                .ok_or_else(|| {
                    ApiError::invalid("`task.probe` must be `list`, `seri` or `scdp`")
                })?;
            let timeout_secs = task
                .get("timeout_secs")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(1.0);
            Ok(Task::Probe { kind, timeout_secs })
        }
        "exituimlk" => Ok(Task::Exituimlk),
        "exit" => Ok(Task::Exit),
        other => Err(ApiError::invalid(format!(
            "unknown `task.kind` value `{other}`"
        ))),
    }
}

// ==============================================================================
// Log forwarding
// ==============================================================================

#[derive(Clone, Copy)]
struct LogBridge {
    callback: unsafe extern "C" fn(line: *const c_char, user_data: *mut c_void),
    user_data: *mut c_void,
}

// Safety: the C++ caller guarantees that `user_data` stays valid (and the
// callback thread-safe) for the whole duration of `auroku_run_task`, and the
// sink never outlives that call.
unsafe impl Send for LogBridge {}
unsafe impl Sync for LogBridge {}

impl LogBridge {
    /// Forward one line to the C++ callback.
    ///
    /// # Safety
    /// `user_data` must still be valid (guaranteed by the FFI contract).
    unsafe fn emit(self, line: &str) {
        let Ok(text) = CString::new(line) else {
            return;
        };
        // Safety: see the `LogBridge` invariants above.
        unsafe { (self.callback)(text.as_ptr(), self.user_data) };
    }

    fn into_sink(self) -> LogSink {
        Arc::new(move |line: &str| {
            // Safety: see the `LogBridge` invariants above.
            unsafe { self.emit(line) };
        })
    }
}

// ==============================================================================
// Exported C ABI
// ==============================================================================

/// Version string of the FFI bridge, e.g. `auroku-ffi 0.1.0 (abi 1)`.
///
/// The caller owns the returned string and must free it with
/// [`auroku_string_free`].
#[no_mangle]
pub extern "C" fn auroku_ffi_version() -> *mut c_char {
    to_c_string(format!(
        "auroku-ffi {} (abi {})",
        env!("CARGO_PKG_VERSION"),
        FFI_ABI_VERSION
    ))
}

/// Enumerate USB devices.
///
/// A negative `vid` / `pid` disables that filter. Returns a JSON envelope whose
/// `data` is an array of
/// `{ vid, pid, bus, address, manufacturer, product, serial, id, display }`.
#[no_mangle]
pub extern "C" fn auroku_scan_devices(vid: i32, pid: i32) -> *mut c_char {
    guarded(|| {
        let vid = (vid >= 0).then_some(vid as u16);
        let pid = (pid >= 0).then_some(pid as u16);
        let devices = scan_devices(vid, pid).map_err(ApiError::from)?;
        Ok(Value::Array(
            devices
                .iter()
                .map(|device| {
                    json!({
                        "vid": device.vid,
                        "pid": device.pid,
                        "bus": device.bus,
                        "address": device.address,
                        "manufacturer": device.manufacturer,
                        "product": device.product,
                        "serial": device.serial,
                        "id": device.id_string(),
                        "display": device.display_name(),
                    })
                })
                .collect(),
        ))
    })
}

/// Execute one protocol operation.
///
/// # Parameters
/// - `request_json`: NUL-terminated UTF-8 JSON object:
///   ```json
///   {
///     "vid": 1154, "pid": 2652,
///     "sn": "SKYKA077394", "interface": null,
///     "ep_out": 4, "ep_in": 132,
///     "verbose": false, "type2": false,
///     "task": { "kind": "status" }
///   }
///   ```
/// - `log_callback`: optional diagnostic sink, called on the calling thread.
/// - `user_data`: opaque pointer passed back to `log_callback`.
///
/// # Safety
/// `request_json` must point to a valid NUL-terminated string. `user_data` must
/// remain valid until this function returns, and `log_callback` must be safe to
/// call from the thread invoking this function.
#[no_mangle]
pub unsafe extern "C" fn auroku_run_task(
    request_json: *const c_char,
    log_callback: AurokuLogCallback,
    user_data: *mut c_void,
) -> *mut c_char {
    guarded(|| {
        if request_json.is_null() {
            return Err(ApiError::invalid("request JSON pointer is null"));
        }
        // Safety: checked for null above; the C++ caller passes a NUL-terminated
        // UTF-8 string.
        let text = unsafe { CStr::from_ptr(request_json) }
            .to_str()
            .map_err(|error| ApiError::invalid(format!("request JSON is not UTF-8: {error}")))?;
        let parsed: Value = serde_json::from_str(text)
            .map_err(|error| ApiError::invalid(format!("invalid request JSON: {error}")))?;
        let request = parsed
            .as_object()
            .ok_or_else(|| ApiError::invalid("request JSON must be an object"))?;

        let config = build_config(request, log_callback, user_data)?;
        let task = build_task(
            request
                .get("task")
                .ok_or_else(|| ApiError::invalid("missing `task` object"))?,
        )?;

        run_task(&config, &task).map_err(ApiError::from)
    })
}

/// Free a string returned by this library.
///
/// # Safety
/// `text` must either be null or a pointer previously returned by this library
/// and not freed yet.
#[no_mangle]
pub unsafe extern "C" fn auroku_string_free(text: *mut c_char) {
    if !text.is_null() {
        // Safety: `text` originates from `CString::into_raw` in this crate.
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_lock_value() {
        let error = build_task(&json!({ "kind": "writelk", "value": "zzzz" })).unwrap_err();
        assert_eq!(error.kind, "ValueError");
    }

    #[test]
    fn parses_probe_defaults() {
        let task = build_task(&json!({ "kind": "probe", "probe": "seri" })).unwrap();
        assert_eq!(
            task,
            Task::Probe {
                kind: ProbeKind::Seri,
                timeout_secs: 1.0
            }
        );
    }

    #[test]
    fn envelope_reports_errors() {
        let pointer = encode(Err(ApiError::invalid("boom")));
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { auroku_string_free(pointer) };
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["kind"], "ValueError");
    }
}
