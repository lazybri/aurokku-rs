#![forbid(unsafe_code)]

//! KDDILCK — Rust port of `kddilck-v2.py`.
//!
//! SERI USB protocol tool for KDDI/au phones:
//!  - UIM lock read / write / clear
//!  - Phone Lock Code (PIN, NV 82) read / reset
//!  - NV 10481 maintenance bit
//!  - ICCID record (0x59) and combined status queries
//!
//! Protocol layout, authentication maths and CLI surface are kept identical to
//! the original Python implementation.

use std::fmt;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use md5::{Digest, Md5};
use rusb::{ConfigDescriptor, Context, DeviceHandle, TransferType, UsbContext};
use serde_json::{json, Value};

const DEFAULT_EP_OUT: u8 = 0x04;
const DEFAULT_EP_IN: u8 = 0x84;

const EXAMPLES: &str = "\
Examples:
  # Check comprehensive device lock and PIN status:
  kddilck --op status

  # Read or clear (unlock) UIM card lock:
  kddilck --op readuimlk
  kddilck --op clearlk

  # Bind UIM lock to a specific card value:
  kddilck --op writelk --val 80e48ed2 --control 1

  # Read or reset phone Lock Code / PIN (NV 82):
  kddilck --op readpin
  kddilck --op resetpin

  # Exit maintenance/PIN mode:
  kddilck --op exit

  # Set NV 10481 maintenance mode bit:
  kddilck --op write10481 --control 1
";

// ==============================================================================
// Errors
// ==============================================================================

#[derive(Debug)]
enum KddiError {
    /// Mirrors Python's `ValueError`.
    Value(String),
    /// Mirrors Python's `RuntimeError`.
    Runtime(String),
    /// Mirrors Python's `TimeoutError`.
    Timeout(String),
}

impl KddiError {
    fn kind(&self) -> &'static str {
        match self {
            KddiError::Value(_) => "ValueError",
            KddiError::Runtime(_) => "RuntimeError",
            KddiError::Timeout(_) => "TimeoutError",
        }
    }
}

impl fmt::Display for KddiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KddiError::Value(msg) | KddiError::Runtime(msg) | KddiError::Timeout(msg) => {
                f.write_str(msg)
            }
        }
    }
}

impl std::error::Error for KddiError {}

impl From<rusb::Error> for KddiError {
    fn from(err: rusb::Error) -> Self {
        match err {
            rusb::Error::Timeout => KddiError::Timeout("USB transfer timed out".to_string()),
            other => KddiError::Runtime(format!("USB error: {other}")),
        }
    }
}

type KddiResult<T> = Result<T, KddiError>;

// ==============================================================================
// 1. Protocol Codec & SN Parsing
// ==============================================================================

/// Split each byte into two nibble bytes: high nibble and low nibble (00..0F).
fn encode_nibbles(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        out.push(b >> 4);
        out.push(b & 0x0F);
    }
    out
}

/// Combine pairs of nibble bytes (00..0F) into original bytes.
fn decode_nibbles(data: &[u8]) -> KddiResult<Vec<u8>> {
    if data.len() % 2 != 0 || data.iter().any(|&b| b > 0x0F) {
        return Err(KddiError::Value(
            "Expected an even number of binary nibble bytes (00..0F)".to_string(),
        ));
    }
    Ok(data.chunks_exact(2).map(|c| (c[0] << 4) | c[1]).collect())
}

/// Decode SERI 0xDC lockcode response bytes into an ASCII string.
fn decode_lockcode_digits(data: &[u8]) -> String {
    let mut out = String::new();
    for &b in data {
        match b {
            0x91..=0x99 => out.push((b - 0x60) as char), // 0x91 -> '1' .. 0x99 -> '9'
            0x9A => out.push('0'),
            0x9B => out.push('*'),
            0x9C => out.push('#'),
            0x9E => out.push('P'),
            0x9F => out.push('-'),
            _ => out.push_str(&format!("[{b:02X}]")),
        }
    }
    out
}

/// Parse a serial number into the 8 raw NV10077 bytes plus a canonical name.
///
/// Accepted formats:
///  - 11 chars: 5 ASCII letters + 6 BCD digits (e.g. `SKYKA077394`)
///  - 16 hex chars: 8 raw NV10077 bytes, stored reversed (e.g. `947307414B594B53`)
fn parse_sn_to_nv10077(sn: &str) -> KddiResult<(Vec<u8>, String)> {
    let s = sn.trim();
    let bytes = s.as_bytes();

    // 1. 11-char format: 5 letters ASCII + 6 digits BCD
    if s.len() == 11
        && s.is_ascii()
        && bytes[..5].iter().all(|b| b.is_ascii_alphabetic())
        && bytes[5..].iter().all(|b| b.is_ascii_digit())
    {
        let mut forward = s[..5].to_ascii_uppercase().into_bytes();
        let bcd = hex::decode(&s[5..])
            .map_err(|e| KddiError::Value(format!("Invalid BCD digits in SN: {e}")))?;
        forward.extend_from_slice(&bcd);

        let nv_raw: Vec<u8> = forward.iter().rev().copied().collect();
        return Ok((nv_raw, s.to_ascii_uppercase()));
    }

    // 2. 16-character hex format
    if s.len() == 16 {
        if let Ok(nv_raw) = hex::decode(s) {
            if nv_raw.len() == 8 {
                // Try to decode to 11-char format for friendly display
                let forward: Vec<u8> = nv_raw.iter().rev().copied().collect();
                let ascii_part = String::from_utf8_lossy(&forward[..5]).to_string();
                let bcd_part = hex::encode(&forward[5..]);

                let canonical_name = if ascii_part.chars().all(|c| c.is_ascii_alphabetic())
                    && bcd_part.chars().all(|c| c.is_ascii_digit())
                {
                    format!("{ascii_part}{bcd_part}")
                } else {
                    s.to_ascii_uppercase()
                };
                return Ok((nv_raw, canonical_name));
            }
        }
    }

    Err(KddiError::Value(format!(
        "Invalid SN format: '{sn}'. Expected 11-char format (e.g. 'SKYKA077394') \
         or 16-char hex NV10077 bytes (e.g. '947307414B594B53')."
    )))
}

// ==============================================================================
// 2. Authentication Calculations
// ==============================================================================

/// Reverse the 8 bits of a single byte.
fn bitrev8(b: u8) -> u8 {
    b.reverse_bits()
}

/// Derive key material `K` from the 8 raw NV 10077 / serial bytes.
fn derive_k(nv_raw: &[u8]) -> KddiResult<[u8; 8]> {
    if nv_raw.len() != 8 {
        return Err(KddiError::Value(format!(
            "Key material must be exactly 8 bytes, got {}",
            nv_raw.len()
        )));
    }
    let s: Vec<u8> = nv_raw.iter().rev().copied().collect(); // sub_46B95D8 forward representation
    Ok([
        bitrev8(s[2]),
        s[5],
        s[7],
        bitrev8(s[3]),
        s[0],
        bitrev8(s[6]),
        bitrev8(s[4]),
        s[1],
    ])
}

fn md5_digest(chunks: &[&[u8]]) -> [u8; 16] {
    let mut hasher = Md5::new();
    for chunk in chunks {
        hasher.update(chunk);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

/// Compute the SERI 0x53 authentication response packet.
fn compute_auth_request(service: &str, challenge: &[u8], nv_raw: &[u8]) -> KddiResult<Vec<u8>> {
    if challenge.len() != 8 {
        return Err(KddiError::Value(format!(
            "Challenge must be 8 decoded bytes, got {}",
            challenge.len()
        )));
    }
    if !matches!(service, "base" | "uimlk" | "2000" | "4000") {
        return Err(KddiError::Value(format!(
            "Unsupported authentication service: {service}"
        )));
    }

    let proof: Vec<u8> = if service == "4000" {
        if nv_raw.len() != 8 {
            return Err(KddiError::Value(format!(
                "Key material must be exactly 8 bytes, got {}",
                nv_raw.len()
            )));
        }
        let s: Vec<u8> = nv_raw.iter().rev().copied().collect();
        let k = [
            s[0],
            s[5],
            s[2],
            s[7],
            s[4],
            s[3],
            bitrev8(s[6]),
            s[1],
        ];
        md5_digest(&[&k[..], challenge])[..8].to_vec()
    } else {
        let k = derive_k(nv_raw)?;
        md5_digest(&[&k[..], challenge])[8..].to_vec()
    };

    let mut packet = Vec::with_capacity(1 + proof.len() * 2);
    packet.push(0x53);
    packet.extend_from_slice(&encode_nibbles(&proof));
    Ok(packet)
}

// ==============================================================================
// 3. Helpers
// ==============================================================================

fn hex_spaced(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Transfer type used for the configured endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XferKind {
    Bulk,
    Interrupt,
}

fn xfer_kind(transfer_type: TransferType) -> XferKind {
    match transfer_type {
        TransferType::Interrupt => XferKind::Interrupt,
        _ => XferKind::Bulk,
    }
}

/// One descriptor per interface (alternate setting 0), with its endpoints.
fn collect_interfaces(config: &ConfigDescriptor) -> Vec<(u8, Vec<(u8, TransferType)>)> {
    let mut result = Vec::new();
    for interface in config.interfaces() {
        let mut descriptors = interface.descriptors();
        if let Some(desc) = descriptors.next() {
            let endpoints = desc
                .endpoint_descriptors()
                .map(|ep| (ep.address(), ep.transfer_type()))
                .collect();
            result.push((desc.interface_number(), endpoints));
        }
    }
    result
}

// ==============================================================================
// 4. SERI USB Device Controller
// ==============================================================================

struct KddiSeriDevice {
    vid: u16,
    pid: u16,
    sn: Option<String>,
    interface_num: Option<u8>,
    ep_out: u8,
    ep_in: u8,
    verbose: bool,
    old: bool,

    handle: Option<DeviceHandle<Context>>,
    claimed: bool,
    nv_material: Vec<u8>,
    rx_buffer: Vec<u8>,
    async_events: Vec<Value>,
    exit_result: Option<Value>,
    awaiting_handshake: bool,
    ep_in_kind: XferKind,
    ep_out_kind: XferKind,
}

#[allow(clippy::too_many_arguments)]
impl KddiSeriDevice {
    fn new(
        vid: u16,
        pid: u16,
        sn: Option<String>,
        interface: Option<u8>,
        ep_out: u8,
        ep_in: u8,
        verbose: bool,
        old: bool,
    ) -> Self {
        Self {
            vid,
            pid,
            sn,
            interface_num: interface,
            ep_out,
            ep_in,
            verbose,
            old,
            handle: None,
            claimed: false,
            nv_material: Vec::new(),
            rx_buffer: Vec::new(),
            async_events: Vec::new(),
            exit_result: None,
            awaiting_handshake: false,
            ep_in_kind: XferKind::Bulk,
            ep_out_kind: XferKind::Bulk,
        }
    }

    // --------------------------------------------------------------------------
    // Raw transport
    // --------------------------------------------------------------------------

    fn log(&self, direction: &str, data: &[u8]) {
        if self.verbose {
            eprintln!("{direction}-> {}", hex_spaced(data));
        }
    }

    fn read_endpoint_in(&self, buf: &mut [u8], timeout: Duration) -> rusb::Result<usize> {
        let handle = self.handle.as_ref().ok_or(rusb::Error::NoDevice)?;
        match self.ep_in_kind {
            XferKind::Interrupt => handle.read_interrupt(self.ep_in, buf, timeout),
            XferKind::Bulk => handle.read_bulk(self.ep_in, buf, timeout),
        }
    }

    fn send(&mut self, data: &[u8]) -> KddiResult<()> {
        self.log("TX", data);

        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| KddiError::Runtime("Device not connected".to_string()))?;

        let timeout = Duration::from_millis(2000);
        let written = match self.ep_out_kind {
            XferKind::Interrupt => handle.write_interrupt(self.ep_out, data, timeout)?,
            XferKind::Bulk => handle.write_bulk(self.ep_out, data, timeout)?,
        };

        if written != data.len() {
            return Err(KddiError::Runtime(format!(
                "Short write: wrote {written}/{} bytes",
                data.len()
            )));
        }
        Ok(())
    }

    fn recv(&mut self, expected_len: usize, timeout: Duration) -> KddiResult<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; 512];

        while self.rx_buffer.len() < expected_len {
            if Instant::now() >= deadline {
                return Err(KddiError::Timeout(format!(
                    "Timeout waiting for {expected_len} bytes; buffered {}",
                    hex_spaced(&self.rx_buffer)
                )));
            }

            match self.read_endpoint_in(&mut buf, Duration::from_millis(500)) {
                Ok(n) => {
                    if n > 0 {
                        self.log("RX", &buf[..n]);
                        self.rx_buffer.extend_from_slice(&buf[..n]);
                        if self.awaiting_handshake {
                            while !self.rx_buffer.is_empty() && self.rx_buffer[0] == 0xAF {
                                self.async_events.push(json!({
                                    "opcode": "af",
                                    "meaning": "pre_echo_delimiter"
                                }));
                                self.rx_buffer.remove(0);
                            }
                        }
                    }
                }
                Err(rusb::Error::Timeout) => {}
                Err(err) => return Err(err.into()),
            }
        }

        let result: Vec<u8> = self.rx_buffer.drain(..expected_len).collect();
        Ok(result)
    }

    /// Drain any residual/stale bytes from the USB IN endpoint and clear local buffer.
    fn drain(&mut self, timeout_ms: u64) {
        self.rx_buffer.clear();
        if self.handle.is_none() {
            return;
        }

        let mut buf = [0u8; 512];
        for _ in 0..20 {
            match self.read_endpoint_in(&mut buf, Duration::from_millis(timeout_ms)) {
                Ok(n) if n > 0 => {
                    let part = buf[..n].to_vec();
                    self.log("RX", &part);
                }
                _ => break,
            }
        }
    }

    fn read_until(&mut self, terminator: u8, limit: usize, timeout: Duration) -> KddiResult<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let mut data = Vec::new();

        while data.len() < limit {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(KddiError::Timeout("Missing response terminator".to_string()));
            }
            data.extend(self.recv(1, remaining)?);
            if data.last() == Some(&terminator) {
                return Ok(data);
            }
        }
        Err(KddiError::Runtime(
            "Response exceeds bounded limit".to_string(),
        ))
    }

    // --------------------------------------------------------------------------
    // Connection management
    // --------------------------------------------------------------------------

    fn connect(&mut self) -> KddiResult<()> {
        let context = Context::new()
            .map_err(|e| KddiError::Runtime(format!("Unable to initialize libusb: {e}")))?;

        let devices = context.devices()?;
        let mut found = None;
        for candidate in devices.iter() {
            let desc = candidate.device_descriptor()?;
            if desc.vendor_id() == self.vid && desc.product_id() == self.pid {
                found = Some(candidate);
                break;
            }
        }
        let device = found.ok_or_else(|| {
            KddiError::Runtime(format!(
                "Device {:04x}:{:04x} not found",
                self.vid, self.pid
            ))
        })?;

        let descriptor = device.device_descriptor()?;
        let handle = device.open()?;

        // Read USB serial string if available
        let device_sn = if descriptor.serial_number_string_index().is_some() {
            match handle.read_serial_number_string_ascii(&descriptor) {
                Ok(sn) => Some(sn),
                Err(err) => {
                    if self.verbose {
                        eprintln!("[WARN] Failed to read iSerialNumber: {err}");
                    }
                    None
                }
            }
        } else {
            None
        };

        let use_sn = if let Some(sn) = self.sn.clone() {
            if let Some(device_sn) = &device_sn {
                if self.verbose && sn.to_uppercase() != device_sn.to_uppercase() {
                    eprintln!(
                        "[WARN] Specified SN ({sn}) differs from USB iSerialNumber ({device_sn})"
                    );
                }
            }
            sn
        } else if let Some(device_sn) = device_sn {
            device_sn
        } else {
            return Err(KddiError::Runtime(
                "Cannot determine device SN. Please provide --sn <11_chars_or_16_hex> explicitly."
                    .to_string(),
            ));
        };

        let (nv_material, canonical_sn) = parse_sn_to_nv10077(&use_sn)?;
        self.nv_material = nv_material;
        self.sn = Some(canonical_sn);

        if self.verbose {
            eprintln!(
                "[INFO] Device SN: {}",
                self.sn.as_deref().unwrap_or_default()
            );
            eprintln!(
                "[INFO] NV10077 Raw: {}",
                hex_spaced(&self.nv_material)
            );
        }

        // Resolve interface
        let config = device.active_config_descriptor()?;
        let interfaces = collect_interfaces(&config);

        if self.interface_num.is_none() {
            for (number, endpoints) in &interfaces {
                let has_out = endpoints.iter().any(|(addr, _)| *addr == self.ep_out);
                let has_in = endpoints.iter().any(|(addr, _)| *addr == self.ep_in);
                if has_out && has_in {
                    self.interface_num = Some(*number);
                    break;
                }
            }
            if self.interface_num.is_none() {
                return Err(KddiError::Runtime(
                    "No interface with the requested endpoints".to_string(),
                ));
            }
        }

        let interface_num = self.interface_num.expect("interface resolved above");
        let endpoints = interfaces
            .iter()
            .find(|(number, _)| *number == interface_num)
            .map(|(_, endpoints)| endpoints)
            .ok_or_else(|| {
                KddiError::Runtime("Selected interface lacks requested endpoints".to_string())
            })?;

        let out_kind = endpoints
            .iter()
            .find(|(addr, _)| *addr == self.ep_out)
            .map(|(_, transfer_type)| xfer_kind(*transfer_type))
            .ok_or_else(|| {
                KddiError::Runtime("Selected interface lacks requested endpoints".to_string())
            })?;
        let in_kind = endpoints
            .iter()
            .find(|(addr, _)| *addr == self.ep_in)
            .map(|(_, transfer_type)| xfer_kind(*transfer_type))
            .ok_or_else(|| {
                KddiError::Runtime("Selected interface lacks requested endpoints".to_string())
            })?;

        self.ep_in_kind = in_kind;
        self.ep_out_kind = out_kind;

        if self.verbose {
            eprintln!(
                "[INFO] Using Interface {interface_num}, EP OUT 0x{:02X}, EP IN 0x{:02X}",
                self.ep_out, self.ep_in
            );
        }

        // Detach a kernel driver if needed (no-op on platforms without support).
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(interface_num)?;
        self.claimed = true;
        self.handle = Some(handle);

        self.drain(50);
        Ok(())
    }

    fn close(&mut self) {
        if let Some(handle) = self.handle.take() {
            if self.claimed {
                if let Some(interface_num) = self.interface_num {
                    let _ = handle.release_interface(interface_num);
                }
                self.claimed = false;
            }
            // Dropping the handle closes the device.
        }
    }

    // --------------------------------------------------------------------------
    // Session & Authentication
    // --------------------------------------------------------------------------

    /// Perform challenge-response authentication for a service.
    fn authenticate_step(&mut self, service_name: &str) -> KddiResult<String> {
        // 1. Request challenge: send 0x52
        self.send(&[0x52])?;
        let resp = self.recv(17, Duration::from_secs(3))?;
        if resp.len() != 17 || resp[0] != 0x52 {
            return Err(KddiError::Runtime(format!(
                "Invalid challenge frame for {service_name}: {}",
                hex_spaced(&resp)
            )));
        }
        let challenge = decode_nibbles(&resp[1..])?;

        // 2. Compute proof & send 0x53
        let auth_packet = compute_auth_request(service_name, &challenge, &self.nv_material)?;
        self.send(&auth_packet)?;

        let auth_resp = self.recv(2, Duration::from_secs(3))?;
        if auth_resp != [0x53, 0x01] {
            return Err(KddiError::Runtime(format!(
                "Authentication failed for {service_name}: {}",
                hex_spaced(&auth_resp)
            )));
        }
        Ok(hex::encode(&challenge))
    }

    /// Establish base session and perform base authentication.
    fn enter_base(&mut self) -> KddiResult<()> {
        self.drain(50);

        // In case the device was left in mask2 (base mode) from an interrupted
        // session, send 0xD8 to clear mask2 so that challenge request 0x52 is
        // not rejected.
        if self.send(&[0xD8]).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
            self.drain(50);
        }

        self.send(&[0x1C, 0x05])?;
        self.awaiting_handshake = true;
        let echo_result = self.recv(2, Duration::from_secs(3));
        self.awaiting_handshake = false;
        let echo = echo_result?;

        if echo != [0x1C, 0x05] {
            return Err(KddiError::Runtime(format!(
                "Base handshake echo mismatch: {}",
                hex_spaced(&echo)
            )));
        }

        if !self.old {
            self.authenticate_step("base")?;
        } else if self.verbose {
            eprintln!(
                "[INFO] --old: skipping base authentication; UIMLK authentication still required"
            );
        }
        Ok(())
    }

    /// Establish session, authenticate base, switch to UIMLK (0xE7) and authenticate.
    fn enter_uimlk(&mut self) -> KddiResult<()> {
        self.enter_base()?;

        // Switch to UIMLK sub-service (0xE7)
        self.send(&[0xE7])?;
        let switch_echo = self.recv(1, Duration::from_secs(3))?;
        if switch_echo != [0xE7] {
            return Err(KddiError::Runtime(format!(
                "Service switch to UIMLK failed: {}",
                hex_spaced(&switch_echo)
            )));
        }

        // UIMLK authentication
        self.authenticate_step("uimlk")?;
        Ok(())
    }

    /// Send base / mask2 exit command (0xD8) to restore phone keypad and normal UI.
    fn exit_base(&mut self) {
        if self.send(&[0xD8]).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Send exit command (0xD8) to return device to normal user mode without reboot.
    fn exit_mode(&mut self) {
        self.exit_base();
    }

    /// Send 0x56 exit command.
    fn exit_uimlk(&mut self) -> KddiResult<Vec<u8>> {
        self.send(&[0x56])?;
        let resp = self.recv(1, Duration::from_secs(3))?;
        if resp != [0x56] {
            return Err(KddiError::Runtime(format!(
                "Invalid exit reply: {}",
                hex::encode(&resp)
            )));
        }
        Ok(resp)
    }

    /// Exit UIMLK sub-service and record the exit response.
    fn finish_uimlk(&mut self) {
        match self.exit_uimlk() {
            Ok(reply) => {
                self.exit_result = Some(json!({
                    "acknowledged": true,
                    "reply": hex::encode(&reply),
                    "may_reboot": true,
                }));
            }
            Err(err) => {
                self.exit_result = Some(json!({
                    "acknowledged": false,
                    "error": err.to_string(),
                    "may_reboot": true,
                }));
            }
        }
    }

    // --------------------------------------------------------------------------
    // UIM Lock Operations
    // --------------------------------------------------------------------------

    /// Send 0x54 to read UIM lock state.
    fn read_state(&mut self) -> KddiResult<Value> {
        self.send(&[0x54])?;
        let resp = self.recv(10, Duration::from_secs(3))?;

        if resp.len() != 10 || resp[0] != 0x74 || (resp[1] != 0 && resp[1] != 1) {
            return Err(KddiError::Runtime(format!(
                "Invalid state reply: {}",
                hex_spaced(&resp)
            )));
        }

        let value = hex::encode(decode_nibbles(&resp[2..])?);
        let binding_cleared = value == "ffffffff";
        Ok(json!({
            "control": resp[1],
            "lock_value": value,
            "binding_cleared": binding_cleared,
            "raw_hex": hex_spaced(&resp),
        }))
    }

    /// Read UIM card lock state (control bit and bound lock value).
    fn read_uimlk(&mut self) -> KddiResult<Value> {
        self.enter_uimlk()?;
        let result = self.read_uimlk_inner();
        self.finish_uimlk();
        result
    }

    fn read_uimlk_inner(&mut self) -> KddiResult<Value> {
        let mut state = self.read_state()?;
        state["operation"] = json!("readuimlk");
        state["success"] = json!(true);
        Ok(state)
    }

    /// Perform a UIMLK mutating command with readback verification.
    fn mutate(
        &mut self,
        operation: &str,
        packet: &[u8],
        expected_reply: &[u8],
        expected_value: &str,
        control: Option<u8>,
    ) -> KddiResult<Value> {
        self.enter_uimlk()?;
        let result = self.mutate_inner(operation, packet, expected_reply, expected_value, control);
        self.finish_uimlk();
        result
    }

    fn mutate_inner(
        &mut self,
        operation: &str,
        packet: &[u8],
        expected_reply: &[u8],
        expected_value: &str,
        control: Option<u8>,
    ) -> KddiResult<Value> {
        let mut result = json!({
            "operation": operation,
            "success": false,
            "persistence_verified": false,
        });

        let before = self.read_state()?;
        result["before"] = before.clone();

        // Send mutation packet
        let reply_result: KddiResult<Vec<u8>> = (|| {
            self.send(packet)?;
            self.recv(2, Duration::from_secs(3))
        })();

        match reply_result {
            Ok(reply) => {
                result["reply"] = json!(hex_spaced(&reply));
                result["command_accepted"] = json!(reply.as_slice() == expected_reply);
            }
            Err(err) => {
                result["command_accepted"] = json!(false);
                result["command_error"] = json!(err.to_string());
            }
        }

        // Read back state (even if the command failed, to detect partial writes)
        match self.read_state() {
            Ok(after) => {
                let exp_control = control.unwrap_or_else(|| {
                    before
                        .get("control")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u8
                });
                let after_value = after
                    .get("lock_value")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let after_control = after
                    .get("control")
                    .and_then(Value::as_u64)
                    .unwrap_or(255) as u8;

                result["after"] = after;
                result["verified"] =
                    json!(after_value == expected_value && after_control == exp_control);
            }
            Err(err) => {
                result["verified"] = json!(false);
                result["readback_error"] = json!(err.to_string());
            }
        }

        let command_accepted = result
            .get("command_accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let verified = result
            .get("verified")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let success = command_accepted && verified;

        result["success"] = json!(success);
        result["partial_write_possible"] = json!(!success);
        Ok(result)
    }

    /// Write UIM lock binding value (4 bytes / 8 hex chars).
    fn write_uimlk(&mut self, lock_val_hex: &str, control: u8) -> KddiResult<Value> {
        let clean: String = lock_val_hex
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let value = hex::decode(&clean)
            .map_err(|e| KddiError::Value(format!("Invalid hex value: {e}")))?;

        if value.len() != 4 || control > 1 {
            return Err(KddiError::Value(
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

    /// Clear/Delete UIM lock to FFFFFFFF (SIM Unlock).
    fn clear_uimlk(&mut self) -> KddiResult<Value> {
        self.mutate("clearlk", &[0x57], &[0x57, 0x01], "ffffffff", None)
    }

    /// Read 10-byte SIM card record (Opcode 0x59, ICCID).
    fn read_record59(&mut self) -> KddiResult<Value> {
        self.enter_uimlk()?;
        let result = self.read_record59_inner();
        self.finish_uimlk();
        result
    }

    fn read_record59_inner(&mut self) -> KddiResult<Value> {
        self.send(&[0x59])?;
        let resp = self.recv(21, Duration::from_secs(3))?;
        if resp.len() != 21 || resp[0] != 0x59 {
            return Err(KddiError::Runtime(format!(
                "Invalid record59 reply: {}",
                hex_spaced(&resp)
            )));
        }

        let decoded = hex_spaced(&decode_nibbles(&resp[1..])?);
        Ok(json!({
            "operation": "readrecord59",
            "success": true,
            "raw_hex": hex_spaced(&resp),
            "decoded_hex": decoded,
        }))
    }

    /// Convenience alias for `read_record59`.
    fn read_iccid(&mut self) -> KddiResult<Value> {
        self.read_record59()
    }

    /// Internal query of record 59 within an active UIMLK session.
    fn query_record59(&mut self) -> KddiResult<Value> {
        self.send(&[0x59])?;
        let resp = self.recv(21, Duration::from_secs(3))?;
        if resp.is_empty() || resp[0] != 0x59 {
            return Err(KddiError::Runtime(
                "Invalid record59 reply".to_string(),
            ));
        }

        let decoded = hex_spaced(&decode_nibbles(&resp[1..])?);
        Ok(json!({
            "decoded_hex": decoded,
            "raw_hex": hex_spaced(&resp),
        }))
    }

    // --------------------------------------------------------------------------
    // Phone Lock Code (PIN) Operations (NV 82)
    // --------------------------------------------------------------------------

    /// Read phone Lock Code / PIN (NV 82) via SERI 0xDC 0x00.
    fn read_lockcode(&mut self, exit_after: bool) -> KddiResult<Value> {
        let result = self.read_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn read_lockcode_inner(&mut self) -> KddiResult<Value> {
        let reply_result: KddiResult<Vec<u8>> = (|| {
            self.send(&[0xDC, 0x00])?;
            self.read_until(0xAA, 256, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.send(&[0xDC, 0x00])?;
                self.read_until(0xAA, 256, Duration::from_secs(3))?
            }
        };

        let index = find_subslice(&reply, &[0xDC, 0x00])
            .ok_or_else(|| invalid_lockcode_reply(&reply))?;
        let frame = &reply[index..];
        if !reply.ends_with(&[0xAA]) || frame.len() < 3 {
            return Err(invalid_lockcode_reply(&reply));
        }

        let digits = decode_lockcode_digits(&frame[2..frame.len() - 1]);
        Ok(json!({
            "operation": "readpin",
            "lockcode": digits,
            "raw_hex": hex_spaced(frame),
            "success": true,
        }))
    }

    /// Reset phone Lock Code / PIN (NV 82) to default '1234' via SERI 0xDC 0x01.
    fn reset_lockcode(&mut self, exit_after: bool) -> KddiResult<Value> {
        let result = self.reset_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn reset_lockcode_inner(&mut self) -> KddiResult<Value> {
        let reply_result: KddiResult<Vec<u8>> = (|| {
            self.send(&[0xDC, 0x01])?;
            self.recv(2, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.send(&[0xDC, 0x01])?;
                self.recv(2, Duration::from_secs(3))?
            }
        };

        if reply != [0xDC, 0x01] {
            return Err(KddiError::Runtime(format!(
                "Reset lockcode failed: {}",
                hex_spaced(&reply)
            )));
        }

        let readback = self.read_lockcode(false)?;
        let lockcode = readback
            .get("lockcode")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let verified = lockcode == "1234";

        Ok(json!({
            "operation": "resetpin",
            "reply": hex_spaced(&reply),
            "lockcode": lockcode,
            "verified": verified,
            "success": true,
        }))
    }

    // --------------------------------------------------------------------------
    // Maintenance Bit (NV 10481)
    // --------------------------------------------------------------------------

    /// Set Maintenance Mode Bit (NV 10481) via service 0xE1 (auth 4000).
    fn write_maintenance_bit(&mut self, control: u8) -> KddiResult<Value> {
        if control > 1 {
            return Err(KddiError::Value(
                "write10481 requires explicit --control 0 or 1".to_string(),
            ));
        }

        self.enter_base()?;
        self.send(&[0xE1])?;
        if self.recv(1, Duration::from_secs(3))? != [0xE1] {
            return Err(KddiError::Runtime(
                "Maintenance service (0xE1) entry failed".to_string(),
            ));
        }
        self.authenticate_step("4000")?;

        self.send(&[0x72, control])?;
        let reply = self.recv(2, Duration::from_secs(3))?;
        let accepted = reply == [0x72, 0x01];

        Ok(json!({
            "operation": "write10481",
            "success": accepted,
            "requested10481": control,
            "reply": hex_spaced(&reply),
            "command_accepted": accepted,
        }))
    }

    // --------------------------------------------------------------------------
    // Session Queries & Status
    // --------------------------------------------------------------------------

    /// Query basic parameters in base mode.
    fn query_basic(&mut self, name: &str) -> KddiResult<Value> {
        match name {
            "version" => {
                self.send(&[0x1B])?;
                let reply = self.recv(5, Duration::from_secs(3))?;
                if reply.is_empty() || reply[0] != 0x1B {
                    return Err(KddiError::Runtime(
                        "Invalid version reply".to_string(),
                    ));
                }
                Ok(json!({ "raw_hex": hex_spaced(&reply) }))
            }
            "lockcode" => {
                self.send(&[0xDC, 0x00])?;
                let reply = self.read_until(0xAA, 256, Duration::from_secs(3))?;
                let decoded = if reply.len() >= 3 {
                    Some(decode_lockcode_digits(&reply[2..reply.len() - 1]))
                } else {
                    None
                };
                Ok(json!({
                    "raw_hex": hex_spaced(&reply),
                    "decoded": decoded,
                }))
            }
            other => Err(KddiError::Value(format!("Unknown query: {other}"))),
        }
    }

    /// Execute multiple queries in a single session without intermediate exits.
    fn batch_read(&mut self, profile: &str) -> KddiResult<Value> {
        let mut report = json!({
            "operation": "batch",
            "profile": profile,
            "success": false,
            "results": [],
            "automatic_exit": false,
        });

        let outcome: KddiResult<()> = (|| {
            self.enter_base()?;
            let mut results: Vec<Value> = Vec::new();

            match profile {
                "basic" => {
                    results.push(with_query("version", self.query_basic("version")?));
                    results.push(with_query("lockcode", self.query_basic("lockcode")?));
                }
                "uimlk" => {
                    self.send(&[0xE7])?;
                    if self.recv(1, Duration::from_secs(3))? != [0xE7] {
                        return Err(KddiError::Runtime("UIM entry failed".to_string()));
                    }
                    self.authenticate_step("uimlk")?;
                    results.push(with_query("state", self.read_state()?));
                    results.push(with_query("record59", self.query_record59()?));
                }
                other => {
                    return Err(KddiError::Value(format!(
                        "Unsupported batch profile: '{other}'. Valid: basic, uimlk"
                    )));
                }
            }

            report["results"] = Value::Array(results);
            report["success"] = json!(true);
            Ok(())
        })();

        if let Err(err) = outcome {
            report["error"] = json!({
                "type": err.kind(),
                "message": err.to_string(),
            });
        }

        Ok(report)
    }

    /// Read comprehensive device status (SN, Lock Code, UIM Lock, ICCID) in one session.
    fn read_status(&mut self) -> KddiResult<Value> {
        let mut report = json!({
            "operation": "status",
            "sn": self.sn.clone(),
            "success": false,
        });
        let mut in_uimlk = false;

        let outcome: KddiResult<()> = (|| {
            self.enter_base()?;

            // 1. Base Version
            match self.query_basic("version") {
                Ok(version) => {
                    report["version"] = version.get("raw_hex").cloned().unwrap_or(Value::Null);
                }
                Err(err) => report["version"] = json!(format!("Error: {err}")),
            }

            // 2. Lock Code (PIN)
            match self.read_lockcode(false) {
                Ok(pin) => {
                    report["lockcode"] = pin.get("lockcode").cloned().unwrap_or(Value::Null);
                }
                Err(err) => report["lockcode"] = json!(format!("Error: {err}")),
            }

            // 3. Switch to UIMLK sub-service
            self.send(&[0xE7])?;
            if self.recv(1, Duration::from_secs(3))? != [0xE7] {
                return Err(KddiError::Runtime(
                    "Switch to UIMLK failed".to_string(),
                ));
            }
            self.authenticate_step("uimlk")?;
            in_uimlk = true;

            // 4. UIM Lock state
            let uim_state = self.read_state()?;
            report["uim_lock"] = json!({
                "control": uim_state.get("control").cloned().unwrap_or(Value::Null),
                "lock_value": uim_state.get("lock_value").cloned().unwrap_or(Value::Null),
                "unlocked": uim_state.get("binding_cleared").cloned().unwrap_or(Value::Null),
            });

            // 5. ICCID / Record 59
            match self.query_record59() {
                Ok(record) => {
                    report["iccid_record"] =
                        record.get("decoded_hex").cloned().unwrap_or(Value::Null);
                }
                Err(err) => report["iccid_record"] = json!(format!("Error: {err}")),
            }

            report["success"] = json!(true);
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

    /// Send exit request (0xD8 for base/PIN, 0x56 for UIMLK) to return device to normal mode.
    fn exit_service_only(&mut self, op: &str) -> KddiResult<Value> {
        match op {
            "exitpin" => {
                self.exit_base();
                Ok(json!({
                    "operation": "exitpin",
                    "success": true,
                    "method": "0xd8",
                }))
            }
            "exituimlk" => {
                self.enter_uimlk()?;
                let reply = self.exit_uimlk()?;
                Ok(json!({
                    "operation": "exituimlk",
                    "success": reply == [0x56],
                    "method": "handshake_and_exit",
                    "reply": hex::encode(&reply),
                }))
            }
            _ => {
                self.exit_base();
                Ok(json!({
                    "operation": "exit",
                    "success": true,
                    "method": "0xd8",
                }))
            }
        }
    }
}

fn invalid_lockcode_reply(reply: &[u8]) -> KddiError {
    KddiError::Runtime(format!("Invalid lockcode reply: {}", hex_spaced(reply)))
}

fn with_query(name: &str, value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.entry("query".to_string())
                .or_insert_with(|| json!(name));
            Value::Object(map)
        }
        other => json!({ "query": name, "value": other }),
    }
}

// ==============================================================================
// 5. CLI & Human-Friendly Presentation
// ==============================================================================

fn str_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn first_display(value: &Value, keys: &[&str]) -> String {
    for key in keys {
        match value.get(*key) {
            Some(Value::String(s)) if !s.is_empty() => return s.clone(),
            Some(Value::String(_)) | Some(Value::Null) | None => continue,
            Some(other) => return other.to_string(),
        }
    }
    String::new()
}

fn display_or_na(value: &Value, key: &str) -> String {
    let text = str_field(value, key);
    if text.is_empty() {
        "N/A".to_string()
    } else {
        text
    }
}

fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// Print a formatted summary of the operation for human reading.
fn print_human_report(op: &str, result: &Value) {
    println!("{}", "=".repeat(60));
    println!("  Operation: {op}");
    println!("{}", "=".repeat(60));

    let success = bool_field(result, "success");
    println!("  Result Status : {}", if success { "SUCCESS" } else { "FAILED" });

    match op {
        "readuimlk" | "readlk" => {
            let value = str_field(result, "lock_value");
            if value == "ffffffff" {
                println!("  UIM Lock      : UNLOCKED / CLEARED (ffffffff)");
            } else {
                println!("  UIM Lock      : LOCKED (Bound Card: {value})");
            }
            println!("  Control Bit   : {}", str_field(result, "control"));
            println!("  Raw Hex       : {}", str_field(result, "raw_hex"));
        }
        "clearlk" | "unlock" => {
            let before = result.get("before").cloned().unwrap_or(Value::Null);
            let after = result.get("after").cloned().unwrap_or(Value::Null);
            println!(
                "  Before Lock   : {} (control: {})",
                str_field(&before, "lock_value"),
                str_field(&before, "control")
            );
            println!(
                "  After Lock    : {} (control: {})",
                str_field(&after, "lock_value"),
                str_field(&after, "control")
            );
            println!("  Verified      : {}", py_bool(bool_field(result, "verified")));
        }
        "writelk" => {
            let before = result.get("before").cloned().unwrap_or(Value::Null);
            let after = result.get("after").cloned().unwrap_or(Value::Null);
            println!(
                "  Before Lock   : {}",
                str_field(&before, "lock_value")
            );
            println!(
                "  Written Lock  : {} (control: {})",
                str_field(&after, "lock_value"),
                str_field(&after, "control")
            );
            println!("  Verified      : {}", py_bool(bool_field(result, "verified")));
        }
        "readiccid" | "readrecord59" => {
            println!("  Decoded ICCID : {}", str_field(result, "decoded_hex"));
            println!("  Raw Response  : {}", str_field(result, "raw_hex"));
        }
        "readpin" => {
            println!("  Phone PIN     : {}", str_field(result, "lockcode"));
        }
        "resetpin" => {
            println!("  New PIN       : {}", str_field(result, "lockcode"));
            println!("  Verified 1234 : {}", py_bool(bool_field(result, "verified")));
        }
        "write10481" | "setmaint" => {
            println!(
                "  Requested     : NV 10481 = {}",
                str_field(result, "requested10481")
            );
            println!(
                "  Accepted      : {}",
                py_bool(bool_field(result, "command_accepted"))
            );
        }
        "status" | "info" => {
            println!("  Device SN     : {}", display_or_na(result, "sn"));
            println!("  Base Version  : {}", display_or_na(result, "version"));
            println!("  Phone PIN     : {}", display_or_na(result, "lockcode"));

            if let Some(uim) = result.get("uim_lock").filter(|value| value.is_object()) {
                let unlocked = bool_field(uim, "unlocked");
                let lock_str = if unlocked {
                    "UNLOCKED".to_string()
                } else {
                    format!("LOCKED ({})", str_field(uim, "lock_value"))
                };
                println!(
                    "  UIM Lock      : {lock_str} (control={})",
                    str_field(uim, "control")
                );
            }

            if result
                .get("iccid_record")
                .map(|value| !value.is_null())
                .unwrap_or(false)
            {
                println!("  SIM ICCID     : {}", str_field(result, "iccid_record"));
            }
        }
        "batch" => {
            println!("  Profile       : {}", str_field(result, "profile"));
            if let Some(items) = result.get("results").and_then(Value::as_array) {
                for item in items {
                    let name = str_field(item, "query");
                    let display = first_display(item, &["decoded", "lock_value", "raw_hex"]);
                    println!("    - {name:<12}: {display}");
                }
            }
        }
        "exituimlk" | "exitpin" | "exit" => {
            println!("  Method        : {}", str_field(result, "method"));
        }
        _ => {}
    }

    if let Some(exit_info) = result.get("exit").filter(|value| value.is_object()) {
        if bool_field(exit_info, "acknowledged") {
            let reply = str_field(exit_info, "reply");
            let reply = if reply.is_empty() {
                "56".to_string()
            } else {
                reply
            };
            println!("  Service Exit  : SUCCESS ({reply})");
        } else if let Some(error) = exit_info.get("error").and_then(Value::as_str) {
            println!("  Service Exit  : FAILED ({error})");
        }
    }

    println!("{}", "=".repeat(60));
}

// ==============================================================================
// 6. Argument Parsing
// ==============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Op {
    #[value(name = "status", alias = "info")]
    Status,
    #[value(name = "readuimlk", alias = "readlk")]
    ReadUimlk,
    #[value(name = "clearlk", alias = "unlock")]
    ClearLk,
    #[value(name = "writelk")]
    WriteLk,
    #[value(name = "readiccid", alias = "readrecord59")]
    ReadIccid,
    #[value(name = "readpin")]
    ReadPin,
    #[value(name = "resetpin")]
    ResetPin,
    #[value(name = "write10481", alias = "setmaint")]
    Write10481,
    #[value(name = "batch")]
    Batch,
    #[value(name = "exituimlk")]
    ExitUimlk,
    #[value(name = "exitpin")]
    ExitPin,
    #[value(name = "exit")]
    Exit,
}

impl Op {
    fn canonical(self) -> &'static str {
        match self {
            Op::Status => "status",
            Op::ReadUimlk => "readuimlk",
            Op::ClearLk => "clearlk",
            Op::WriteLk => "writelk",
            Op::ReadIccid => "readiccid",
            Op::ReadPin => "readpin",
            Op::ResetPin => "resetpin",
            Op::Write10481 => "write10481",
            Op::Batch => "batch",
            Op::ExitUimlk => "exituimlk",
            Op::ExitPin => "exitpin",
            Op::Exit => "exit",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Profile {
    Basic,
    Uimlk,
}

impl Profile {
    fn as_str(self) -> &'static str {
        match self {
            Profile::Basic => "basic",
            Profile::Uimlk => "uimlk",
        }
    }
}

fn parse_hex_u16(value: &str) -> Result<u16, String> {
    let trimmed = value.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u16::from_str_radix(digits, 16).map_err(|e| format!("invalid hex value '{value}': {e}"))
}

#[derive(Debug, Parser)]
#[command(name = "kddilck", about = "KDDILCK", after_help = EXAMPLES)]
struct Cli {
    /// Operation to perform on the device
    #[arg(long, value_enum, required = true)]
    op: Op,

    /// USB Vendor ID in hex (default: 0482)
    #[arg(long, value_parser = parse_hex_u16, default_value = "0482")]
    vid: u16,

    /// USB Product ID in hex (default: 0a5c)
    #[arg(long, value_parser = parse_hex_u16, default_value = "0a5c")]
    pid: u16,

    /// Device serial number: 11-char format (e.g. SKYKA077394) or 16-hex
    /// NV10077. Auto-detected if omitted.
    #[arg(long)]
    sn: Option<String>,

    /// 4-byte hex value for writelk (e.g. 80e48ed2 or ffffffff)
    #[arg(long = "val", visible_alias = "value")]
    val: Option<String>,

    /// Control bit (0 or 1); required for write10481, writelk defaults to 1
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=1))]
    control: Option<u8>,

    /// USB Interface number (auto-detected if omitted)
    #[arg(long)]
    interface: Option<u8>,

    /// Profile for batch query (basic or uimlk)
    #[arg(long, value_enum, default_value = "basic")]
    profile: Profile,

    /// Enable verbose debug log (prints TX-> and RX-> hex frames)
    #[arg(short, long)]
    verbose: bool,

    /// Use SOY04-style handshake without base 52/53 authentication (experimental)
    #[arg(long)]
    old: bool,

    /// Output result in JSON format
    #[arg(long)]
    json: bool,
}

// ==============================================================================
// 7. Main
// ==============================================================================

fn dispatch(device: &mut KddiSeriDevice, cli: &Cli) -> KddiResult<Value> {
    match cli.op {
        Op::Status => device.read_status(),
        Op::ReadUimlk => device.read_uimlk(),
        Op::ClearLk => device.clear_uimlk(),
        Op::WriteLk => {
            let value = cli.val.clone().unwrap_or_default();
            device.write_uimlk(&value, cli.control.unwrap_or(1))
        }
        Op::ReadIccid => device.read_iccid(),
        Op::ReadPin => device.read_lockcode(true),
        Op::ResetPin => device.reset_lockcode(true),
        Op::Write10481 => {
            let control = cli.control.ok_or_else(|| {
                KddiError::Value("write10481 requires explicit --control 0 or 1".to_string())
            })?;
            device.write_maintenance_bit(control)
        }
        Op::Batch => device.batch_read(cli.profile.as_str()),
        Op::ExitUimlk => device.exit_service_only("exituimlk"),
        Op::ExitPin => device.exit_service_only("exitpin"),
        Op::Exit => device.exit_service_only("exit"),
    }
}

fn run_session(device: &mut KddiSeriDevice, cli: &Cli) -> KddiResult<Value> {
    device.connect()?;

    let mut result = dispatch(device, cli)?;
    result["exit"] = device.exit_result.clone().unwrap_or(Value::Null);
    result["async_events"] = Value::Array(device.async_events.clone());
    Ok(result)
}

fn main() {
    let cli = Cli::parse();

    // Input validation
    if cli.op == Op::WriteLk && cli.val.is_none() {
        eprintln!("Error: --op writelk requires --val <8_hex_chars> (e.g. --val 80e48ed2)");
        std::process::exit(1);
    }
    if cli.op == Op::Write10481 && cli.control.is_none() {
        eprintln!("Error: --op write10481 requires --control 0 or 1");
        std::process::exit(1);
    }

    let mut device = KddiSeriDevice::new(
        cli.vid,
        cli.pid,
        cli.sn.clone(),
        cli.interface,
        DEFAULT_EP_OUT,
        DEFAULT_EP_IN,
        cli.verbose,
        cli.old,
    );

    let outcome = run_session(&mut device, &cli);
    device.close();

    match outcome {
        Ok(result) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                print_human_report(cli.op.canonical(), &result);
            }

            if !result
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                std::process::exit(1);
            }
        }
        Err(err) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "error": err.kind(),
                        "message": err.to_string(),
                    }))
                    .unwrap_or_default()
                );
            } else {
                eprintln!("Error ({}): {err}", err.kind());
            }
            std::process::exit(1);
        }
    }
}

// ==============================================================================
// Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_roundtrip() {
        let data = [0x00, 0x0F, 0xA5, 0xFF, 0x53, 0x01];
        let encoded = encode_nibbles(&data);
        assert_eq!(encoded.len(), data.len() * 2);
        assert!(encoded.iter().all(|&b| b <= 0x0F));
        assert_eq!(decode_nibbles(&encoded).unwrap(), data);
    }

    #[test]
    fn nibble_rejects_invalid() {
        assert!(decode_nibbles(&[0x01]).is_err());
        assert!(decode_nibbles(&[0x10, 0x00]).is_err());
    }

    #[test]
    fn bitrev8_works() {
        assert_eq!(bitrev8(0b0000_0001), 0b1000_0000);
        assert_eq!(bitrev8(0b1111_0000), 0b0000_1111);
    }

    #[test]
    fn parse_sn_11_char_and_hex_agree() {
        let (nv_a, name_a) = parse_sn_to_nv10077("SKYKA077394").unwrap();
        assert_eq!(name_a, "SKYKA077394");
        assert_eq!(hex::encode(&nv_a), "947307414b594b53");

        let (nv_b, name_b) = parse_sn_to_nv10077("947307414B594B53").unwrap();
        assert_eq!(nv_a, nv_b);
        assert_eq!(name_b, "SKYKA077394");
    }

    #[test]
    fn parse_sn_rejects_garbage() {
        assert!(parse_sn_to_nv10077("").is_err());
        assert!(parse_sn_to_nv10077("SKYKA07739").is_err());
        assert!(parse_sn_to_nv10077("ZZZZZZZZZZZZZZZZ").is_err());
    }

    #[test]
    fn auth_request_layout() {
        let (nv, _) = parse_sn_to_nv10077("SKYKA077394").unwrap();
        let challenge = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

        for service in ["base", "uimlk", "2000", "4000"] {
            let packet = compute_auth_request(service, &challenge, &nv).unwrap();
            assert_eq!(packet.len(), 17);
            assert_eq!(packet[0], 0x53);
            assert!(packet[1..].iter().all(|&b| b <= 0x0F));
        }
    }

    #[test]
    fn auth_request_rejects_bad_service_and_challenge() {
        let (nv, _) = parse_sn_to_nv10077("SKYKA077394").unwrap();
        assert!(compute_auth_request("bogus", &[0u8; 8], &nv).is_err());
        assert!(compute_auth_request("base", &[0u8; 7], &nv).is_err());
    }

    #[test]
    fn lockcode_digits_decode() {
        // 1 2 3 4 * # 0 P -
        let raw = [0x91, 0x92, 0x93, 0x94, 0x9B, 0x9C, 0x9A, 0x9E, 0x9F];
        assert_eq!(decode_lockcode_digits(&raw), "1234*#0P-");
        assert_eq!(decode_lockcode_digits(&[0x80]), "[80]");
    }

    #[test]
    fn find_subslice_works() {
        assert_eq!(find_subslice(&[0x00, 0xDC, 0x00, 0xAA], &[0xDC, 0x00]), Some(1));
        assert_eq!(find_subslice(&[0x00, 0xAA], &[0xDC]), None);
    }

    #[test]
    fn hex_spaced_format() {
        assert_eq!(hex_spaced(&[0x74, 0x01, 0xFF]), "74 01 ff");
        assert_eq!(hex_spaced(&[]), "");
    }
}
