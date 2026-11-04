//! Error type shared by every protocol operation.
//!
//! Mirrors the desktop `AuRokkuError` variants (root `src/lib.rs:41-116`) so the
//! `kind()` strings used in JSON (`ValueError`, `RuntimeError`, ...) stay
//! identical. The only semantic change is `std::error::Error` ->
//! `core::error::Error` (stable since Rust 1.81) and the removal of the
//! rusb-specific `From` impl (that belongs to the desktop transport layer).

use alloc::string::String;
use core::fmt;

/// Errors surfaced by the protocol layer.
///
/// Variants intentionally mirror Python's `ValueError` / `RuntimeError` /
/// `TimeoutError` so the CLI and GUI can report the same error kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuRokkuError {
    /// Mirrors Python's `ValueError`.
    Value(String),
    /// Mirrors Python's `RuntimeError`.
    Runtime(String),
    /// Mirrors Python's `TimeoutError`.
    Timeout(String),
    /// The device is enumerated but cannot be opened until another driver is
    /// bound to it (Windows only; kept so both sides can match the variant).
    Driver(String),
}

impl AuRokkuError {
    /// Name of the equivalent Python exception.
    pub fn kind(&self) -> &'static str {
        match self {
            AuRokkuError::Value(_) => "ValueError",
            AuRokkuError::Runtime(_) => "RuntimeError",
            AuRokkuError::Timeout(_) => "TimeoutError",
            AuRokkuError::Driver(_) => "DriverError",
        }
    }
}

impl fmt::Display for AuRokkuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuRokkuError::Value(msg)
            | AuRokkuError::Runtime(msg)
            | AuRokkuError::Timeout(msg)
            | AuRokkuError::Driver(msg) => f.write_str(msg),
        }
    }
}

impl core::error::Error for AuRokkuError {}

/// Result type used by every public protocol operation.
pub type AuRokkuResult<T> = Result<T, AuRokkuError>;
