//! Protocol operations: the twelve `--op` tasks and their typed reports.
//!
//! Line-by-line port of the desktop `impl AuRokkuDevice` operation methods
//! (`src/lib.rs:1314-1815`) plus `run_task` dispatch (`src/lib.rs:2200-2235`).
//! The only changes are:
//! - transport calls go through [`crate::FrameIo`] / [`UsbTransport`];
//! - JSON objects become typed [`Report`] values;
//! - `f64` probe timeouts become `u32` milliseconds (docs/plan/08 R1).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use crate::codec::{
    decode_lockcode_digits, decode_nibbles, encode_nibbles, find_subslice, hex_spaced,
};
use crate::error::{AuRokkuError, AuRokkuResult};
use crate::model::{BatchProfile, ProbeKind, Task};
use crate::report::{
    BatchError, BatchReport, BatchResult, ExitBaseReport, ExitUimlkReport, FieldValue, IccidReport,
    MaintenanceReport, MutationReport, PinReport, ProbeReport, ReadUimlkReport, Report,
    ResetPinReport, StatusReport, UimLockState, UimLockSummary,
};
use crate::session::{Session, TaskOutcome};
use crate::transport::{UsbTransport, DEFAULT_TIMEOUT, DRAIN_TIMEOUT};

/// Probe response read limit (bytes), same as the original 512-byte buffer.
const PROBE_LIMIT: usize = 512;

impl<T: UsbTransport> Session<T> {
    /// Execute one task and return its typed report plus the trailing session
    /// data (`exit`, `async_events`) the desktop app attaches to every result.
    ///
    /// Original: `run_task_with` + `dispatch_task` (`src/lib.rs:2200-2235`).
    ///
    /// Note: [`Task::Probe`] with [`ProbeKind::List`] is a host-side descriptor
    /// operation and is not implemented by the protocol core; the platform must
    /// build that report from its USB descriptor view.
    pub fn run(&mut self, task: &Task) -> AuRokkuResult<TaskOutcome> {
        let report = match task {
            Task::Status => Report::Status(self.read_status()?),
            Task::ReadUimlk => Report::ReadUimlk(self.read_uimlk()?),
            Task::Clearlk => Report::Mutation(self.clear_uimlk()?),
            Task::Writelk { value, control } => {
                Report::Mutation(self.write_uimlk(value, *control)?)
            }
            Task::ReadIccid => Report::Iccid(self.read_iccid()?),
            Task::Readpin => Report::Pin(self.read_lockcode(true)?),
            Task::Resetpin => Report::ResetPin(self.reset_lockcode(true)?),
            Task::MaintenanceBit { control } => {
                Report::Maintenance(self.write_maintenance_bit(*control)?)
            }
            Task::Batch { profile } => Report::Batch(self.batch_read(*profile)?),
            Task::Probe { kind, timeout_ms } => Report::Probe(self.probe(*kind, *timeout_ms)?),
            Task::Exituimlk => self.exit_service_only(true)?,
            Task::Exit => self.exit_service_only(false)?,
        };
        Ok(TaskOutcome {
            report,
            exit: self.exit_result.take(),
            async_events: self.io.take_async_events(),
        })
    }

    // ----------------------------------------------------------------------
    // UIM lock operations (original `src/lib.rs:1314-1513`)
    // ----------------------------------------------------------------------

    /// Send `0x54` and parse the `0x74` state frame.
    ///
    /// Original: `read_state` (`src/lib.rs:1319-1338`).
    fn read_state(&mut self) -> AuRokkuResult<UimLockState> {
        self.io.send(&[0x54])?;
        let resp = self.io.recv(10, DEFAULT_TIMEOUT)?;

        if resp.len() != 10 || resp[0] != 0x74 || (resp[1] != 0 && resp[1] != 1) {
            return Err(AuRokkuError::Runtime(format!(
                "Invalid state reply: {}",
                hex_spaced(&resp)
            )));
        }

        let value = hex::encode(decode_nibbles(&resp[2..])?);
        let binding_cleared = value == "ffffffff";
        Ok(UimLockState {
            control: resp[1],
            lock_value: value,
            binding_cleared,
            raw_hex: hex_spaced(&resp),
        })
    }

    /// Read UIM lock state inside a UIMLK session.
    ///
    /// Original: `read_uimlk` (`src/lib.rs:1341-1354`).
    fn read_uimlk(&mut self) -> AuRokkuResult<ReadUimlkReport> {
        self.enter_uimlk()?;
        let result = self.read_state();
        self.finish_uimlk();
        Ok(ReadUimlkReport { state: result? })
    }

    /// Mutating command with readback verification (shared helper).
    ///
    /// Original: `mutate` (`src/lib.rs:1356-1369`).
    fn mutate(
        &mut self,
        operation: &'static str,
        packet: &[u8],
        expected_reply: &[u8],
        expected_value: &str,
        control: Option<u8>,
    ) -> AuRokkuResult<MutationReport> {
        self.enter_uimlk()?;
        let result = self.mutate_inner(operation, packet, expected_reply, expected_value, control);
        self.finish_uimlk();
        result
    }

    /// Original: `mutate_inner` (`src/lib.rs:1371-1440`).
    fn mutate_inner(
        &mut self,
        operation: &'static str,
        packet: &[u8],
        expected_reply: &[u8],
        expected_value: &str,
        control: Option<u8>,
    ) -> AuRokkuResult<MutationReport> {
        let before = self.read_state()?;

        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.io.send(packet)?;
            self.io.recv(2, DEFAULT_TIMEOUT)
        })();

        let mut report = MutationReport {
            operation,
            before: before.clone(),
            reply: None,
            command_accepted: false,
            command_error: None,
            after: None,
            verified: false,
            readback_error: None,
            success: false,
            partial_write_possible: false,
            persistence_verified: false,
        };

        match reply_result {
            Ok(reply) => {
                report.reply = Some(hex_spaced(&reply));
                report.command_accepted = reply.as_slice() == expected_reply;
            }
            Err(err) => {
                report.command_accepted = false;
                report.command_error = Some(err.to_string());
            }
        }

        let expected_control = control.unwrap_or(before.control);
        match self.read_state() {
            Ok(after) => {
                report.verified =
                    after.lock_value == expected_value && after.control == expected_control;
                report.after = Some(after);
            }
            Err(err) => {
                report.verified = false;
                report.readback_error = Some(err.to_string());
            }
        }

        report.success = report.command_accepted && report.verified;
        report.partial_write_possible = !report.success;
        Ok(report)
    }

    /// Write UIM lock binding value (4 bytes / 8 hex chars).
    ///
    /// Original: `write_uimlk` (`src/lib.rs:1443-1467`).
    fn write_uimlk(&mut self, lock_val_hex: &str, control: u8) -> AuRokkuResult<MutationReport> {
        let clean = lock_val_hex.replace(' ', "");
        let value = hex::decode(&clean)
            .map_err(|e| AuRokkuError::Value(format!("Invalid hex value: {e}")))?;

        if value.len() != 4 || control > 1 {
            return Err(AuRokkuError::Value(
                "Expected 4 value bytes and control 0 or 1".to_string(),
            ));
        }

        let mut packet = Vec::with_capacity(10);
        packet.push(0x72);
        packet.push(control);
        packet.extend_from_slice(&encode_nibbles(&value));

        let expected_value = hex::encode(&value);
        self.mutate(
            "writelk",
            &packet,
            &[0x72, 0x01],
            &expected_value,
            Some(control),
        )
    }

    /// Clear/Delete UIM lock to `FFFFFFFF`.
    ///
    /// Original: `clear_uimlk` (`src/lib.rs:1470-1472`).
    fn clear_uimlk(&mut self) -> AuRokkuResult<MutationReport> {
        self.mutate("clearlk", &[0x57], &[0x57, 0x01], "ffffffff", None)
    }

    /// Read the SIM ICCID record (`0x59`) inside a UIMLK session.
    ///
    /// Original: `read_iccid` (`src/lib.rs:1475-1499`).
    fn read_iccid(&mut self) -> AuRokkuResult<IccidReport> {
        self.enter_uimlk()?;
        let result = self.read_iccid_inner();
        self.finish_uimlk();
        result
    }

    fn read_iccid_inner(&mut self) -> AuRokkuResult<IccidReport> {
        self.io.send(&[0x59])?;
        let resp = self.io.recv(21, DEFAULT_TIMEOUT)?;

        if resp.len() != 21 || resp[0] != 0x59 {
            return Err(AuRokkuError::Runtime(format!(
                "Invalid record59 reply: {}",
                hex_spaced(&resp)
            )));
        }

        Ok(IccidReport {
            raw_hex: hex_spaced(&resp),
            decoded_hex: hex_spaced(&decode_nibbles(&resp[1..])?),
        })
    }

    /// Internal record-59 query (no session management).
    ///
    /// Original: `query_record59` (`src/lib.rs:1502-1513`).
    fn query_record59(&mut self) -> AuRokkuResult<BatchResult> {
        self.io.send(&[0x59])?;
        let resp = self.io.recv(21, DEFAULT_TIMEOUT)?;
        if resp.is_empty() || resp[0] != 0x59 {
            return Err(AuRokkuError::Runtime("Invalid record59 reply".to_string()));
        }

        Ok(BatchResult::Record59 {
            decoded_hex: hex_spaced(&decode_nibbles(&resp[1..])?),
            raw_hex: hex_spaced(&resp),
        })
    }

    // ----------------------------------------------------------------------
    // Phone lock code (PIN, NV 82) operations (original `src/lib.rs:1515-1605`)
    // ----------------------------------------------------------------------

    /// Read phone Lock Code / PIN via SERI `0xDC 0x00`.
    ///
    /// Original: `read_lockcode` (`src/lib.rs:1520-1557`).
    fn read_lockcode(&mut self, exit_after: bool) -> AuRokkuResult<PinReport> {
        let result = self.read_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn read_lockcode_inner(&mut self) -> AuRokkuResult<PinReport> {
        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.io.send(&[0xDC, 0x00])?;
            self.io.read_until(0xAA, 256, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.io.send(&[0xDC, 0x00])?;
                self.io.read_until(0xAA, 256, DEFAULT_TIMEOUT)?
            }
        };

        let index =
            find_subslice(&reply, &[0xDC, 0x00]).ok_or_else(|| invalid_lockcode_reply(&reply))?;
        let frame = &reply[index..];
        if !reply.ends_with(&[0xAA]) || frame.len() < 3 {
            return Err(invalid_lockcode_reply(&reply));
        }

        let digits = decode_lockcode_digits(&frame[2..frame.len() - 1]);
        Ok(PinReport {
            lockcode: digits,
            raw_hex: hex_spaced(frame),
        })
    }

    /// Reset phone Lock Code / PIN to default `1234` via SERI `0xDC 0x01`.
    ///
    /// Original: `reset_lockcode` (`src/lib.rs:1560-1605`).
    fn reset_lockcode(&mut self, exit_after: bool) -> AuRokkuResult<ResetPinReport> {
        let result = self.reset_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn reset_lockcode_inner(&mut self) -> AuRokkuResult<ResetPinReport> {
        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.io.send(&[0xDC, 0x01])?;
            self.io.recv(2, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.io.send(&[0xDC, 0x01])?;
                self.io.recv(2, DEFAULT_TIMEOUT)?
            }
        };

        if reply != [0xDC, 0x01] {
            return Err(AuRokkuError::Runtime(format!(
                "Reset lockcode failed: {}",
                hex_spaced(&reply)
            )));
        }

        let readback = self.read_lockcode(false)?;
        let verified = readback.lockcode == "1234";

        Ok(ResetPinReport {
            reply: hex_spaced(&reply),
            lockcode: readback.lockcode,
            verified,
        })
    }

    // ----------------------------------------------------------------------
    // Maintenance bit (original `src/lib.rs:1607-1645`)
    // ----------------------------------------------------------------------

    /// Set Maintenance Mode Bit (NV 10481) via service `0xE1` (auth 4000).
    ///
    /// Original: `write_maintenance_bit` (`src/lib.rs:1612-1645`).
    fn write_maintenance_bit(&mut self, control: u8) -> AuRokkuResult<MaintenanceReport> {
        if control > 1 {
            return Err(AuRokkuError::Value(
                "maintenance_bit requires --control 0 or 1".to_string(),
            ));
        }

        self.enter_base()?;
        self.io.send(&[0xE1])?;
        if self.io.recv(1, DEFAULT_TIMEOUT)? != [0xE1] {
            return Err(AuRokkuError::Runtime(
                "Maintenance service (0xE1) entry failed".to_string(),
            ));
        }
        self.authenticate_step("4000")?;

        let packet = [0x72, control];
        let expected = [0x72, 0x01];

        self.io.send(&packet)?;
        let reply = self.io.recv(2, DEFAULT_TIMEOUT)?;
        let accepted = reply == expected;

        Ok(MaintenanceReport {
            maintenance_bit: control,
            reply: hex_spaced(&reply),
            command_accepted: accepted,
            success: accepted,
        })
    }

    // ----------------------------------------------------------------------
    // Session queries, status, batch (original `src/lib.rs:1647-1794`)
    // ----------------------------------------------------------------------

    /// Query basic parameters in base mode.
    ///
    /// Original: `query_basic` (`src/lib.rs:1652-1677`).
    fn query_basic(&mut self, name: &str) -> AuRokkuResult<BasicQuery> {
        match name {
            "version" => {
                self.io.send(&[0x1B])?;
                let reply = self.io.recv(5, DEFAULT_TIMEOUT)?;
                if reply.is_empty() || reply[0] != 0x1B {
                    return Err(AuRokkuError::Runtime("Invalid version reply".to_string()));
                }
                Ok(BasicQuery::Version {
                    raw_hex: hex_spaced(&reply),
                })
            }
            "lockcode" => {
                self.io.send(&[0xDC, 0x00])?;
                let reply = self.io.read_until(0xAA, 256, DEFAULT_TIMEOUT)?;
                let decoded = if reply.len() >= 3 {
                    Some(decode_lockcode_digits(&reply[2..reply.len() - 1]))
                } else {
                    None
                };
                Ok(BasicQuery::Lockcode {
                    raw_hex: hex_spaced(&reply),
                    decoded,
                })
            }
            other => Err(AuRokkuError::Value(format!("Unknown query: {other}"))),
        }
    }

    /// Execute multiple queries in a single session without intermediate exits.
    ///
    /// Original: `batch_read` (`src/lib.rs:1680-1727`). Errors are recorded in
    /// the report (`success: false`, `error`) and still return `Ok`.
    fn batch_read(&mut self, profile: BatchProfile) -> AuRokkuResult<BatchReport> {
        let mut report = BatchReport {
            profile,
            success: false,
            results: Vec::new(),
            error: None,
            automatic_exit: false,
        };

        let outcome: AuRokkuResult<Vec<BatchResult>> = (|| {
            self.enter_base()?;
            let mut results: Vec<BatchResult> = Vec::new();

            match profile {
                BatchProfile::Basic => {
                    match self.query_basic("version")? {
                        BasicQuery::Version { raw_hex } => {
                            results.push(BatchResult::Version { raw_hex });
                        }
                        BasicQuery::Lockcode { .. } => unreachable!("version query"),
                    }
                    match self.query_basic("lockcode")? {
                        BasicQuery::Lockcode { raw_hex, decoded } => {
                            results.push(BatchResult::Lockcode { raw_hex, decoded });
                        }
                        BasicQuery::Version { .. } => unreachable!("lockcode query"),
                    }
                }
                BatchProfile::Uimlk => {
                    self.io.send(&[0xE7])?;
                    if self.io.recv(1, DEFAULT_TIMEOUT)? != [0xE7] {
                        return Err(AuRokkuError::Runtime("UIM entry failed".to_string()));
                    }
                    self.authenticate_step("uimlk")?;
                    results.push(BatchResult::State(self.read_state()?));
                    results.push(self.query_record59()?);
                }
            }

            Ok(results)
        })();

        match outcome {
            Ok(results) => {
                report.results = results;
                report.success = true;
            }
            Err(err) => {
                report.error = Some(BatchError {
                    kind: err.kind(),
                    message: err.to_string(),
                });
            }
        }

        Ok(report)
    }

    /// Read comprehensive device status (SN, Lock Code, UIM Lock, ICCID).
    ///
    /// Original: `read_status` (`src/lib.rs:1730-1794`).
    fn read_status(&mut self) -> AuRokkuResult<StatusReport> {
        let mut report = StatusReport {
            sn: self.sn.clone(),
            version: None,
            lockcode: None,
            uim_lock: None,
            iccid_record: None,
        };
        let mut in_uimlk = false;

        let outcome: AuRokkuResult<()> = (|| {
            self.enter_base()?;

            // 1. Base version
            report.version = Some(match self.query_basic("version") {
                Ok(BasicQuery::Version { raw_hex }) => FieldValue::Value(raw_hex),
                Ok(BasicQuery::Lockcode { .. }) => unreachable!("version query"),
                Err(err) => FieldValue::Error(format!("Error: {err}")),
            });

            // 2. Lock Code (PIN)
            report.lockcode = Some(match self.read_lockcode_inner() {
                Ok(pin) => FieldValue::Value(pin.lockcode),
                Err(err) => FieldValue::Error(format!("Error: {err}")),
            });

            // 3. Switch to UIMLK sub-service
            self.io.send(&[0xE7])?;
            if self.io.recv(1, DEFAULT_TIMEOUT)? != [0xE7] {
                return Err(AuRokkuError::Runtime("Switch to UIMLK failed".to_string()));
            }
            self.authenticate_step("uimlk")?;
            in_uimlk = true;

            // 4. UIM Lock state
            let state = self.read_state()?;
            report.uim_lock = Some(UimLockSummary {
                control: state.control,
                lock_value: state.lock_value.clone(),
                unlocked: state.binding_cleared,
            });

            // 5. ICCID / Record 59
            report.iccid_record = Some(match self.query_record59() {
                Ok(BatchResult::Record59 { decoded_hex, .. }) => FieldValue::Value(decoded_hex),
                Ok(_) => FieldValue::Error("Error: unexpected record59 result".to_string()),
                Err(err) => FieldValue::Error(format!("Error: {err}")),
            });

            Ok(())
        })();

        if in_uimlk {
            self.finish_uimlk();
        } else {
            self.exit_base();
        }

        outcome?;
        Ok(report)
    }

    /// Send exit request (`0xD8` for base/PIN, `0x56` for UIMLK).
    ///
    /// Original: `exit_service_only` (`src/lib.rs:1797-1815`).
    fn exit_service_only(&mut self, exituimlk: bool) -> AuRokkuResult<Report> {
        if exituimlk {
            self.enter_uimlk()?;
            let reply = self.exit_uimlk()?;
            Ok(Report::ExitUimlk(ExitUimlkReport {
                reply: hex::encode(&reply),
                success: reply == [0x56],
                method: "handshake_and_exit",
            }))
        } else {
            self.exit_base();
            Ok(Report::ExitBase(ExitBaseReport {
                success: true,
                method: "0xd8",
            }))
        }
    }

    // ----------------------------------------------------------------------
    // Active probe (original `src/lib.rs:1079-1142`, descriptor part excluded)
    // ----------------------------------------------------------------------

    /// Actively probe the SERI / SCDP services.
    ///
    /// Original: `probe` (`src/lib.rs:1079-1142`). The `usb`/`selection` report
    /// fields are produced by the platform from its descriptor view; this method
    /// only performs the byte-level exchange.
    fn probe(&mut self, kind: ProbeKind, timeout_ms: u32) -> AuRokkuResult<ProbeReport> {
        if kind == ProbeKind::List {
            return Err(AuRokkuError::Value(
                "probe list is a host-side descriptor operation".to_string(),
            ));
        }

        self.io.drain(DRAIN_TIMEOUT);

        match kind {
            ProbeKind::Seri => {
                let request = vec![0x1C, 0x05];
                self.io.send(&request)?;
                let response = self.io.read_probe_response(
                    Duration::from_millis(u64::from(timeout_ms)),
                    PROBE_LIMIT,
                )?;
                let matched = response.starts_with(&request);

                let mut report = ProbeReport {
                    probe: ProbeKind::Seri,
                    request_hex: Some(hex_spaced(&request)),
                    response_hex: Some(hex_spaced(&response)),
                    responsive: !response.is_empty(),
                    matched: Some(matched),
                    cleanup_hex: None,
                    response_length: None,
                    success: matched,
                };

                if matched && self.vid != 0x0FCE {
                    self.io.send(&[0xD8])?;
                    report.cleanup_hex = Some("d8".to_string());
                }
                Ok(report)
            }
            ProbeKind::Scdp => {
                let request = vec![0x8E];
                self.io.send(&request)?;
                let response = self.io.read_probe_response(
                    Duration::from_millis(u64::from(timeout_ms)),
                    PROBE_LIMIT,
                )?;

                Ok(ProbeReport {
                    probe: ProbeKind::Scdp,
                    request_hex: Some(hex_spaced(&request)),
                    response_hex: Some(hex_spaced(&response)),
                    responsive: !response.is_empty(),
                    matched: None,
                    cleanup_hex: None,
                    response_length: Some(response.len()),
                    success: !response.is_empty(),
                })
            }
            ProbeKind::List => unreachable!("handled above"),
        }
    }
}

/// Result of a `query_basic` call (original returned a JSON object).
enum BasicQuery {
    Version {
        raw_hex: String,
    },
    Lockcode {
        raw_hex: String,
        decoded: Option<String>,
    },
}

/// Original: `invalid_lockcode_reply` (`src/lib.rs:1818-1820`).
fn invalid_lockcode_reply(reply: &[u8]) -> AuRokkuError {
    AuRokkuError::Runtime(format!("Invalid lockcode reply: {}", hex_spaced(reply)))
}
