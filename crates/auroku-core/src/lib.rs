#![no_std]
#![forbid(unsafe_code)]

//! auRokku protocol core (`no_std` + `alloc`).
//!
//! This crate contains the host-independent half of the auRokku USB protocol:
//! byte codecs, Type2Auth (MD5 challenge/response), the UIMLK session state
//! machine, every operation payload/sequence, and typed reports.
//!
//! It intentionally knows nothing about USB descriptors, libusb/rusb, clap,
//! serde_json, files or threads. The backend is supplied through the
//! [`UsbTransport`] trait, so the same protocol code runs on:
//!
//! - the desktop app (`rusb`, see `src/transport_rusb.rs` of the root crate);
//! - an MCU USB host stack (RP2354 firmware) - see `docs/plan/06`..`10`.
//!
//! Migration discipline (docs/plan/08 §6.1):
//! 1. the core does not know that USB exists: it only writes frames, reads
//!    frames, waits and asks for a monotonic clock;
//! 2. operation sequences are copied line-by-line from the original
//!    `src/lib.rs` (`enter_base`, `read_state`, `mutate`, ...); each item keeps
//!    a comment with the original line range;
//! 3. the desktop JSON contract is *not* produced here - it is rebuilt from the
//!    typed [`Report`] values by the desktop adapter, so the CLI/GUI output is
//!    unchanged. Typed reports exist because serde_json is not `no_std`.
//!
//! The core requires `alloc` (it uses `Vec`/`String` for frame buffers and hex
//! messages, matching the original code). A fixed-buffer variant is possible
//! later; RP2354 has 520 KB of SRAM, so `alloc` is not a constraint today.

extern crate alloc;

pub mod codec;
pub mod error;
pub mod frame;
pub mod mock;
pub mod model;
pub mod ops;
pub mod report;
pub mod session;
pub mod transport;
pub mod type2auth;

pub use codec::{
    decode_lockcode_digits, decode_nibbles, encode_nibbles, find_subslice, hex_spaced,
};
pub use error::{AuRokkuError, AuRokkuResult};
pub use frame::{AsyncEvent, FrameIo};
pub use model::{BatchProfile, ProbeKind, Task};
pub use report::{
    BatchError, BatchReport, BatchResult, ExitBaseReport, ExitOutcome, ExitUimlkReport, FieldValue,
    IccidReport, MaintenanceReport, MutationReport, PinReport, ProbeReport, ReadUimlkReport,
    Report, ResetPinReport, StatusReport, UimLockState, UimLockSummary,
};
pub use session::{Session, SessionConfig, TaskOutcome};
pub use transport::{
    UsbTransport, DEFAULT_TIMEOUT, DEFAULT_WRITE_TIMEOUT, DRAIN_TIMEOUT, ENDPOINT_CHUNK,
    MAX_DRAIN_READS, READ_SLICE_TIMEOUT,
};
pub use type2auth::{
    bitrev8, compute_type2auth_request, derive_type2auth_key, md5_digest, parse_sn_to_type2auth,
};
