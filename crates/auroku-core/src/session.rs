//! Session establishment, authentication and exit handling.
//!
//! Line-by-line port of the desktop session state machine
//! (`src/lib.rs:1164-1312`): enter base (`0xD8` pre-exit + `0x1C 0x05`
//! handshake), switch to UIMLK (`0xE7`), Type2Auth challenge-response
//! (`0x52`/`0x53`), and both exit paths (`0x56` / `0xD8`).
//!
//! The only structural change is that the transport is a generic
//! [`UsbTransport`] and deadlines come from its monotonic clock.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use crate::codec::{decode_nibbles, hex_spaced};
use crate::error::{AuRokkuError, AuRokkuResult};
use crate::frame::{AsyncEvent, FrameIo};
use crate::report::{ExitOutcome, Report};
use crate::transport::{UsbTransport, DEFAULT_TIMEOUT, DRAIN_TIMEOUT};
use crate::type2auth::{compute_type2auth_request, parse_sn_to_type2auth};

/// A completed operation plus the trailing session data the desktop app attaches
/// to every result JSON (`exit` and `async_events`).
#[derive(Debug, Clone)]
pub struct TaskOutcome {
    pub report: Report,
    pub exit: Option<ExitOutcome>,
    pub async_events: Vec<AsyncEvent>,
}

/// Everything the protocol core needs to know about a device session.
///
/// USB descriptors, interface selection and iSerialNumber reads belong to the
/// platform layer; the platform passes the chosen SN in here.
#[derive(Clone, Debug, Default)]
pub struct SessionConfig {
    /// USB vendor id (used for the `0x0FCE` exit-command special case).
    pub vid: u16,
    /// Whether `--type2` base authentication is enabled.
    pub type2: bool,
    /// Serial number (11-character or 16-hex form). Required for UIMLK.
    pub sn: Option<String>,
}

/// A protocol session over one [`UsbTransport`].
pub struct Session<T: UsbTransport> {
    pub(crate) io: FrameIo<T>,
    pub(crate) vid: u16,
    pub(crate) type2: bool,
    pub(crate) sn: Option<String>,
    pub(crate) material: Option<Vec<u8>>,
    pub(crate) exit_result: Option<ExitOutcome>,
}

impl<T: UsbTransport> Session<T> {
    /// Build a session. The SN, when present, is parsed into Type2Auth material
    /// immediately (same validation/errors as the original `open_device`).
    pub fn new(transport: T, config: SessionConfig) -> AuRokkuResult<Self> {
        let (sn, material) = match config.sn {
            Some(sn) => {
                let (material, canonical) = parse_sn_to_type2auth(&sn)?;
                (Some(canonical), Some(material))
            }
            None => (None, None),
        };
        Ok(Self {
            io: FrameIo::new(transport),
            vid: config.vid,
            type2: config.type2,
            sn,
            material,
            exit_result: None,
        })
    }

    /// Canonical SN (uppercase), if one was supplied.
    pub fn sn(&self) -> Option<&str> {
        self.sn.as_deref()
    }

    /// Exit result recorded by [`Self::finish_uimlk`], if any.
    pub fn exit_result(&self) -> Option<&ExitOutcome> {
        self.exit_result.as_ref()
    }

    /// Borrow the underlying transport (e.g. to inspect endpoint state).
    pub fn transport(&mut self) -> &mut T {
        self.io.transport()
    }

    /// Consume the session and return the transport.
    pub fn into_transport(self) -> T {
        self.io.into_inner()
    }

    /// Type2Auth material for the configured SN.
    fn material(&self) -> AuRokkuResult<&[u8]> {
        self.material.as_deref().ok_or_else(|| {
            AuRokkuError::Runtime(
                "Cannot determine device SN. Please provide --sn <11_chars_or_16_hex> explicitly."
                    .to_string(),
            )
        })
    }

    /// Type2Auth challenge-response for one service.
    ///
    /// Original: `authenticate_step` (`src/lib.rs:1164-1216`). Handles up to four
    /// extra `0xE7` acknowledgements before the UIMLK challenge.
    pub(crate) fn authenticate_step(&mut self, service_name: &str) -> AuRokkuResult<String> {
        self.io.send(&[0x52])?;

        let deadline = self.io.transport().now_ms() + DEFAULT_TIMEOUT.as_millis() as u64;
        let mut resp = self.io.recv(17, DEFAULT_TIMEOUT)?;
        let mut extra_echoes = 0;

        while service_name == "uimlk" && resp.first() == Some(&0xE7) {
            if extra_echoes >= 4 {
                return Err(AuRokkuError::Runtime(
                    "Too many E7 acknowledgements before UIMLK challenge".to_string(),
                ));
            }

            let remaining = deadline.saturating_sub(self.io.transport().now_ms());
            if remaining == 0 {
                return Err(AuRokkuError::Timeout(
                    "Timeout completing UIMLK challenge after E7 acknowledgement".to_string(),
                ));
            }

            extra_echoes += 1;
            // The original pushes an async_events JSON entry; the frame layer only
            // records the typed event (the adapter renders the JSON).
            self.io.push_async_event(AsyncEvent::ExtraUimlkAck);

            let tail = self.io.recv(1, Duration::from_millis(remaining))?;
            resp.remove(0);
            resp.extend_from_slice(&tail);
        }

        if resp.len() != 17 || resp[0] != 0x52 {
            return Err(AuRokkuError::Runtime(format!(
                "Invalid challenge frame for {service_name}: {}",
                hex_spaced(&resp)
            )));
        }
        let challenge = decode_nibbles(&resp[1..])?;

        let auth_packet = compute_type2auth_request(service_name, &challenge, self.material()?)?;
        self.io.send(&auth_packet)?;

        let auth_resp = self.io.recv(2, DEFAULT_TIMEOUT)?;
        if auth_resp != [0x53, 0x01] {
            return Err(AuRokkuError::Runtime(format!(
                "Authentication failed for {service_name}: {}",
                hex_spaced(&auth_resp)
            )));
        }

        Ok(hex::encode(&challenge))
    }

    /// Establish the base session, optionally authenticating base (`--type2`).
    ///
    /// Original: `enter_base` (`src/lib.rs:1219-1246`).
    pub(crate) fn enter_base(&mut self) -> AuRokkuResult<()> {
        self.io.drain(DRAIN_TIMEOUT);

        if self.vid != 0x0FCE && self.io.send(&[0xD8]).is_ok() {
            self.io.transport().delay(Duration::from_millis(50));
            self.io.drain(DRAIN_TIMEOUT);
        }

        self.io.send(&[0x1C, 0x05])?;
        self.io.begin_handshake();
        let echo_result = self.io.recv(2, DEFAULT_TIMEOUT);
        self.io.end_handshake();
        let echo = echo_result?;

        if echo != [0x1C, 0x05] {
            return Err(AuRokkuError::Runtime(format!(
                "Base handshake echo mismatch: {}",
                hex_spaced(&echo)
            )));
        }

        if self.type2 {
            self.authenticate_step("base")?;
        }
        Ok(())
    }

    /// Enter base, switch to UIMLK (`0xE7`) and authenticate it.
    ///
    /// Original: `enter_uimlk` (`src/lib.rs:1249-1263`).
    pub(crate) fn enter_uimlk(&mut self) -> AuRokkuResult<()> {
        self.enter_base()?;

        self.io.send(&[0xE7])?;
        let switch_echo = self.io.recv(1, DEFAULT_TIMEOUT)?;
        if switch_echo != [0xE7] {
            return Err(AuRokkuError::Runtime(format!(
                "Service switch to UIMLK failed: {}",
                hex_spaced(&switch_echo)
            )));
        }

        self.authenticate_step("uimlk")?;
        Ok(())
    }

    /// Send the base/mask2 exit command (`0xD8`).
    ///
    /// Original: `exit_base` (`src/lib.rs:1266-1270`).
    pub(crate) fn exit_base(&mut self) {
        if self.io.send(&[0xD8]).is_ok() {
            self.io.transport().delay(Duration::from_millis(50));
        }
    }

    /// Leave base mode for non-`0x0FCE` devices (returns the command used).
    ///
    /// Original: `exit_mode` (`src/lib.rs:1273-1279`).
    pub(crate) fn exit_mode(&mut self) -> Option<Vec<u8>> {
        if self.vid != 0x0FCE {
            self.exit_base();
            return Some(alloc::vec![0xD8]);
        }
        None
    }

    /// Send the UIMLK exit command (`0x56`).
    ///
    /// Original: `exit_uimlk` (`src/lib.rs:1282-1292`).
    pub(crate) fn exit_uimlk(&mut self) -> AuRokkuResult<Vec<u8>> {
        self.io.send(&[0x56])?;
        let resp = self.io.recv(1, DEFAULT_TIMEOUT)?;
        if resp != [0x56] {
            return Err(AuRokkuError::Runtime(format!(
                "Invalid exit reply: {}",
                hex::encode(&resp)
            )));
        }
        Ok(resp)
    }

    /// Exit the UIMLK sub-service and record the exit response without failing
    /// the operation (the original only sets `exit_result`).
    ///
    /// Original: `finish_uimlk` (`src/lib.rs:1295-1312`).
    pub(crate) fn finish_uimlk(&mut self) {
        match self.exit_uimlk() {
            Ok(reply) => {
                self.exit_result = Some(ExitOutcome {
                    acknowledged: true,
                    reply: Some(hex::encode(&reply)),
                    error: None,
                    may_reboot: true,
                });
            }
            Err(err) => {
                self.exit_result = Some(ExitOutcome {
                    acknowledged: false,
                    reply: None,
                    error: Some(err.to_string()),
                    may_reboot: true,
                });
            }
        }
    }
}
