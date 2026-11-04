//! Typed protocol reports.
//!
//! These structs are the `no_std` replacement for the `serde_json::Value`
//! results built by the desktop `src/lib.rs` (see docs/plan/08 §6.6). Field
//! names and value shapes deliberately mirror the existing JSON keys; the
//! desktop adapter in the root crate converts them back to `Value` so the
//! CLI/GUI output contract does not change.

use alloc::string::String;
use alloc::vec::Vec;

use crate::model::{BatchProfile, ProbeKind};

/// Trailing `exit` object produced by `finish_uimlk` / `exit_base`.
///
/// JSON shape: `{ "acknowledged": bool, "reply": "...", "error": "...",
/// "may_reboot": true }` - note `reply` and `error` are mutually exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitOutcome {
    pub acknowledged: bool,
    pub reply: Option<String>,
    pub error: Option<String>,
    pub may_reboot: bool,
}

/// UIM lock state (`0x74` frame), also the `before`/`after` payload.
///
/// JSON shape: `{ "control": 0|1, "lock_value": "12345678",
/// "binding_cleared": bool, "raw_hex": "74 ..." }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UimLockState {
    pub control: u8,
    pub lock_value: String,
    pub binding_cleared: bool,
    pub raw_hex: String,
}

/// A status field that the original renders as either the value or the literal
/// string `"Error: {message}"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue<T> {
    Value(T),
    Error(String),
}

impl<T> FieldValue<T> {
    /// Value if present, `None` for the error variant.
    pub fn value(&self) -> Option<&T> {
        match self {
            FieldValue::Value(value) => Some(value),
            FieldValue::Error(_) => None,
        }
    }

    /// Error message if present.
    pub fn error(&self) -> Option<&str> {
        match self {
            FieldValue::Value(_) => None,
            FieldValue::Error(message) => Some(message),
        }
    }
}

/// UIM lock summary inside the status report.
///
/// JSON shape: `{ "control": u8, "lock_value": "...", "unlocked": bool }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UimLockSummary {
    pub control: u8,
    pub lock_value: String,
    pub unlocked: bool,
}

/// Comprehensive status (`--op status`).
///
/// JSON shape: `{ "operation": "status", "sn": ..., "success": true,
/// "version": ..., "lockcode": ..., "uim_lock": {...}, "iccid_record": ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub sn: Option<String>,
    pub version: Option<FieldValue<String>>,
    pub lockcode: Option<FieldValue<String>>,
    pub uim_lock: Option<UimLockSummary>,
    pub iccid_record: Option<FieldValue<String>>,
}

/// `--op readuimlk` payload (the `success`/`operation` wrapper is added by the
/// adapter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadUimlkReport {
    pub state: UimLockState,
}

/// `--op writelk` / `--op clearlk` mutation with readback verification.
///
/// JSON shape: `{ "operation": ..., "success": bool,
/// "persistence_verified": false, "before": {...}, "reply": "...",
/// "command_accepted": bool, "command_error": "...", "after": {...},
/// "verified": bool, "readback_error": "...",
/// "partial_write_possible": bool }` - optional keys appear only on the
/// corresponding branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationReport {
    pub operation: &'static str,
    pub before: UimLockState,
    pub reply: Option<String>,
    pub command_accepted: bool,
    pub command_error: Option<String>,
    pub after: Option<UimLockState>,
    pub verified: bool,
    pub readback_error: Option<String>,
    pub success: bool,
    pub partial_write_possible: bool,
    /// The original initialises this to `false` and never updates it.
    pub persistence_verified: bool,
}

/// `--op readiccid`.
///
/// JSON shape: `{ "operation": "readiccid", "success": true,
/// "raw_hex": "...", "decoded_hex": "..." }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IccidReport {
    pub raw_hex: String,
    pub decoded_hex: String,
}

/// `--op readpin`.
///
/// JSON shape: `{ "operation": "readpin", "lockcode": "1234",
/// "raw_hex": "dc 00 ... aa", "success": true }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinReport {
    pub lockcode: String,
    pub raw_hex: String,
}

/// `--op resetpin`.
///
/// JSON shape: `{ "operation": "resetpin", "reply": "dc01", "lockcode":
/// "1234", "verified": true, "success": true }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetPinReport {
    pub reply: String,
    pub lockcode: String,
    pub verified: bool,
}

/// `--op maintenance_bit`.
///
/// JSON shape: `{ "operation": "maintenance_bit", "success": bool,
/// "maintenance_bit": u8, "reply": "...", "command_accepted": bool }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceReport {
    pub maintenance_bit: u8,
    pub reply: String,
    pub command_accepted: bool,
    pub success: bool,
}

/// One entry of the batch result list. The original wraps each JSON object with
/// a `"query"` key; that key is implicit in the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchResult {
    Version {
        raw_hex: String,
    },
    Lockcode {
        raw_hex: String,
        decoded: Option<String>,
    },
    State(UimLockState),
    Record59 {
        decoded_hex: String,
        raw_hex: String,
    },
}

/// Batch error object (`{ "type": "ValueError", "message": "..." }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchError {
    pub kind: &'static str,
    pub message: String,
}

/// `--op batch`.
///
/// JSON shape: `{ "operation": "batch", "profile": "basic", "success": bool,
/// "results": [...], "automatic_exit": false, "error": {...} }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchReport {
    pub profile: BatchProfile,
    pub success: bool,
    pub results: Vec<BatchResult>,
    pub error: Option<BatchError>,
    pub automatic_exit: bool,
}

/// Active probe payload (SERI/SCDP). Descriptor fields (`usb`, `selection`) are
/// the platform's responsibility and are merged by the desktop/firmware layer.
///
/// JSON shape: `{ "probe": "seri", "request_hex": "...", "response_hex": "...",
/// "responsive": bool, "matched": bool, "success": bool,
/// "cleanup_hex": "d8", "response_length": n }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub probe: ProbeKind,
    pub request_hex: Option<String>,
    pub response_hex: Option<String>,
    pub responsive: bool,
    pub matched: Option<bool>,
    pub cleanup_hex: Option<String>,
    pub response_length: Option<usize>,
    pub success: bool,
}

/// `--op exituimlk`.
///
/// JSON shape: `{ "operation": "exituimlk", "success": true,
/// "method": "handshake_and_exit", "reply": "56" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitUimlkReport {
    pub reply: String,
    pub success: bool,
    pub method: &'static str,
}

/// `--op exit` (base / mask2, `0xD8`).
///
/// JSON shape: `{ "operation": "exit", "success": true, "method": "0xd8" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitBaseReport {
    pub success: bool,
    pub method: &'static str,
}

/// A completed operation's typed report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Status(StatusReport),
    ReadUimlk(ReadUimlkReport),
    Mutation(MutationReport),
    Iccid(IccidReport),
    Pin(PinReport),
    ResetPin(ResetPinReport),
    Maintenance(MaintenanceReport),
    Batch(BatchReport),
    Probe(ProbeReport),
    ExitUimlk(ExitUimlkReport),
    ExitBase(ExitBaseReport),
}

impl Report {
    /// Operation name (`status`, `readuimlk`, ...).
    pub fn op_name(&self) -> &'static str {
        match self {
            Report::Status(_) => "status",
            Report::ReadUimlk(_) => "readuimlk",
            Report::Mutation(report) => report.operation,
            Report::Iccid(_) => "readiccid",
            Report::Pin(_) => "readpin",
            Report::ResetPin(_) => "resetpin",
            Report::Maintenance(_) => "maintenance_bit",
            Report::Batch(_) => "batch",
            Report::Probe(_) => "probe",
            Report::ExitUimlk(_) => "exituimlk",
            Report::ExitBase(_) => "exit",
        }
    }
}
