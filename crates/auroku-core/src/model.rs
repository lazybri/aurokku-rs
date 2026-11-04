//! Operation/task model shared by the desktop adapter and the firmware UI.
//!
//! Mirrors the desktop `Task` / `ProbeKind` / `BatchProfile` (`src/lib.rs:420-511`)
//! without the clap derive attributes. One deliberate change (docs/plan/08 R1):
//! `Task::Probe` carries `timeout_ms: u32` instead of `timeout_secs: f64`, so the
//! core never calls soft-float `f64` math on Cortex-M33.

use alloc::string::String;

/// One protocol operation, mirroring the CLI `--op` values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task {
    Status,
    ReadUimlk,
    Clearlk,
    Writelk { value: String, control: u8 },
    ReadIccid,
    Readpin,
    Resetpin,
    MaintenanceBit { control: u8 },
    Batch { profile: BatchProfile },
    Probe { kind: ProbeKind, timeout_ms: u32 },
    Exituimlk,
    Exit,
}

impl Task {
    /// Operation name used in reports (same spelling as the CLI).
    pub fn op_name(&self) -> &'static str {
        match self {
            Task::Status => "status",
            Task::ReadUimlk => "readuimlk",
            Task::Clearlk => "clearlk",
            Task::Writelk { .. } => "writelk",
            Task::ReadIccid => "readiccid",
            Task::Readpin => "readpin",
            Task::Resetpin => "resetpin",
            Task::MaintenanceBit { .. } => "maintenance_bit",
            Task::Batch { .. } => "batch",
            Task::Probe { .. } => "probe",
            Task::Exituimlk => "exituimlk",
            Task::Exit => "exit",
        }
    }
}

/// Probe flavour (`--probe-kind`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProbeKind {
    List,
    Seri,
    Scdp,
}

impl ProbeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProbeKind::List => "list",
            ProbeKind::Seri => "seri",
            ProbeKind::Scdp => "scdp",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "list" => Some(ProbeKind::List),
            "seri" => Some(ProbeKind::Seri),
            "scdp" => Some(ProbeKind::Scdp),
            _ => None,
        }
    }
}

/// Batch query profile (`--profile`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BatchProfile {
    Basic,
    Uimlk,
}

impl BatchProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            BatchProfile::Basic => "basic",
            BatchProfile::Uimlk => "uimlk",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "basic" => Some(BatchProfile::Basic),
            "uimlk" => Some(BatchProfile::Uimlk),
            _ => None,
        }
    }
}
