//! Firmware protocol layer: wraps `auroku-core` sessions.
//!
//! The platform layer (RP2354 USB host bring-up) will supply a
//! [`UsbTransport`] implementation; on the host we supply
//! `auroku_core::mock::MockTransport`. This module contains no USB stack code
//! itself, which is what makes the firmware testable before the board exists.
//!
//! Note: the MVP executes one task synchronously (matching the desktop
//! behaviour). docs/plan/06 §6.2 requires moving this onto a second core /
//! async executor later; the [`ProtoOutcome`] boundary is designed to make that
//! change local to the platform layer.

use alloc::string::String;

use auroku_core::{AuRokkuError, Session, SessionConfig, Task, TaskOutcome, UsbTransport};

/// Device information the platform learned from the USB descriptors.
///
/// `sn` is the iSerialNumber string (needed for Type2Auth); the platform must
/// read it during enumeration (docs/plan/08 R14). When absent the user has to
/// enter an SN on the OLED UI (docs/plan/09 §5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vid: u16,
    pub sn: Option<String>,
}

/// Everything needed to create a core session.
#[derive(Clone, Debug, Default)]
pub struct ProtoConfig {
    pub vid: u16,
    pub type2: bool,
    pub sn: Option<String>,
}

/// Result of one task, tagged with the task so the UI can render the right page.
#[derive(Clone, Debug)]
pub struct ProtoOutcome {
    pub task: Task,
    pub result: Result<TaskOutcome, AuRokkuError>,
}

/// Execute one task against the attached phone.
///
/// The transport is borrowed, so the caller keeps ownership across attach /
/// detach. `Session::new` parses the SN into Type2Auth material exactly like the
/// desktop app did.
pub fn run_task<T: UsbTransport + ?Sized>(
    transport: &mut T,
    config: &ProtoConfig,
    task: &Task,
) -> ProtoOutcome {
    let session_config = SessionConfig {
        vid: config.vid,
        type2: config.type2,
        sn: config.sn.clone(),
    };
    let result = Session::new(transport, session_config).and_then(|mut session| session.run(task));
    ProtoOutcome {
        task: task.clone(),
        result,
    }
}
