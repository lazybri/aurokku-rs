#![forbid(unsafe_code)]

//! auRokku — Rust port of `auRokku.py`.
//!
//! SERI / Type2Auth USB service tool for au (KDDI) phones:
//!  - UIM lock read / write / clear
//!  - Phone Lock Code (PIN, NV 82) read / reset
//!  - NV 10481 maintenance bit
//!  - ICCID record (0x59), comprehensive status and batch queries
//!  - USB layout / SERI / SCDP probing
//!
//! Protocol layout, authentication maths, JSON shape and CLI surface are kept
//! identical to the original Python implementation.

use std::fmt;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use md5::{Digest, Md5};
use rusb::{ConfigDescriptor, Context, Device, DeviceHandle, Direction, TransferType, UsbContext};
use serde_json::{json, Value};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

const EXAMPLES: &str = "\
Examples:
  # Inspect USB layout (no device handle/SN required):
  auroku --op probe --vid 0482 --pid 0a5c

  # Active SERI echo / SCDP probe:
  auroku --op probe --vid 0482 --pid 0a5c --probe-kind seri

  # Check comprehensive device lock and PIN status:
  auroku --op status --vid 0482 --pid 0a5c

  # Read or clear (unlock) UIM card lock:
  auroku --op readuimlk --vid 0482 --pid 0a5c
  auroku --op clearlk --vid 0482 --pid 0a5c

  # Bind UIM lock to a specific card value:
  auroku --op writelk --vid 0482 --pid 0a5c --val 80e48ed2 --control 1

  # Read or reset phone Lock Code / PIN (NV 82):
  auroku --op readpin --vid 0482 --pid 0a5c
  auroku --op resetpin --vid 0482 --pid 0a5c

  # Set NV 10481 maintenance mode bit:
  auroku --op maintenance_bit --vid 0482 --pid 0a5c --control 1

  # Exit maintenance / UIM service:
  auroku --op exit --vid 0482 --pid 0a5c
";

// ==============================================================================
// Errors
// ==============================================================================

#[derive(Debug)]
enum AuRokkuError {
    /// Mirrors Python's `ValueError`.
    Value(String),
    /// Mirrors Python's `RuntimeError`.
    Runtime(String),
    /// Mirrors Python's `TimeoutError`.
    Timeout(String),
}

impl AuRokkuError {
    fn kind(&self) -> &'static str {
        match self {
            AuRokkuError::Value(_) => "ValueError",
            AuRokkuError::Runtime(_) => "RuntimeError",
            AuRokkuError::Timeout(_) => "TimeoutError",
        }
    }
}

impl fmt::Display for AuRokkuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuRokkuError::Value(msg) | AuRokkuError::Runtime(msg) | AuRokkuError::Timeout(msg) => {
                f.write_str(msg)
            }
        }
    }
}

impl std::error::Error for AuRokkuError {}

impl From<rusb::Error> for AuRokkuError {
    fn from(err: rusb::Error) -> Self {
        match err {
            rusb::Error::Timeout => AuRokkuError::Timeout("USB transfer timed out".to_string()),
            other => AuRokkuError::Runtime(format!("USB error: {other}")),
        }
    }
}

type AuRokkuResult<T> = Result<T, AuRokkuError>;

// ==============================================================================
// 1. Protocol Codec, SN Parsing & CLI helpers
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
fn decode_nibbles(data: &[u8]) -> AuRokkuResult<Vec<u8>> {
    if !data.len().is_multiple_of(2) || data.iter().any(|&b| b > 0x0F) {
        return Err(AuRokkuError::Value(
            "Expected an even number of binary nibble bytes (00..0F)".to_string(),
        ));
    }
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| (c[0] << 4) | c[1])
        .collect())
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

/// Parse a serial number into the 8 raw Type2Auth bytes plus the canonical name.
///
/// Matches `parse_sn_to_type2auth` in the Python source:
///  - 16 hex chars: used verbatim as raw Type2Auth material.
///  - otherwise: first 5 chars are ASCII (upper-cased) followed by hex digits;
///    the resulting bytes are stored reversed.
fn parse_sn_to_type2auth(sn_str: &str) -> AuRokkuResult<(Vec<u8>, String)> {
    let s = sn_str.trim();

    if s.len() == 16 {
        let auth_raw = hex::decode(s)
            .map_err(|e| AuRokkuError::Value(format!("Invalid Type2Auth hex SN '{s}': {e}")))?;
        return Ok((auth_raw, s.to_uppercase()));
    }

    if !s.is_ascii() {
        return Err(AuRokkuError::Value(format!(
            "SN must be ASCII to derive Type2Auth material: '{s}'"
        )));
    }

    let split = s.len().min(5);
    let mut forward = s[..split].to_ascii_uppercase().into_bytes();
    let tail = hex::decode(&s[split..])
        .map_err(|e| AuRokkuError::Value(format!("Invalid hex digits in SN '{s}': {e}")))?;
    forward.extend_from_slice(&tail);
    forward.reverse();
    Ok((forward, s.to_uppercase()))
}

/// `argparse`-style hex USB endpoint parser.
fn parse_usb_endpoint(value: &str) -> Result<u8, String> {
    let trimmed = value.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let endpoint =
        u16::from_str_radix(digits, 16).map_err(|_| format!("Invalid USB endpoint: {value:?}"))?;
    if endpoint > 0xFF {
        return Err("USB endpoint must be between 00 and FF".to_string());
    }
    Ok(endpoint as u8)
}

fn parse_hex_u16(value: &str) -> Result<u16, String> {
    let trimmed = value.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u16::from_str_radix(digits, 16).map_err(|e| format!("invalid hex value '{value}': {e}"))
}

// ==============================================================================
// 2. Type2Auth Calculations
// ==============================================================================

/// Reverse the 8 bits of a single byte.
fn bitrev8(b: u8) -> u8 {
    b.reverse_bits()
}

/// Derive key material `K` from the 8 raw Type2Auth bytes.
fn derive_type2auth_key(auth_raw: &[u8]) -> AuRokkuResult<[u8; 8]> {
    if auth_raw.len() != 8 {
        return Err(AuRokkuError::Value(format!(
            "Key material must be exactly 8 bytes, got {}",
            auth_raw.len()
        )));
    }
    let s: Vec<u8> = auth_raw.iter().rev().copied().collect();
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
fn compute_type2auth_request(
    service: &str,
    challenge: &[u8],
    auth_raw: &[u8],
) -> AuRokkuResult<Vec<u8>> {
    if challenge.len() != 8 {
        return Err(AuRokkuError::Value(format!(
            "Challenge must be 8 decoded bytes, got {}",
            challenge.len()
        )));
    }
    if !matches!(service, "base" | "uimlk" | "2000" | "4000") {
        return Err(AuRokkuError::Value(format!(
            "Unsupported authentication service: {service}"
        )));
    }

    let proof: Vec<u8> = if service == "4000" {
        if auth_raw.len() != 8 {
            return Err(AuRokkuError::Value(format!(
                "Key material must be exactly 8 bytes, got {}",
                auth_raw.len()
            )));
        }
        let s: Vec<u8> = auth_raw.iter().rev().copied().collect();
        let k = [s[0], s[5], s[2], s[7], s[4], s[3], bitrev8(s[6]), s[1]];
        md5_digest(&[&k[..], challenge])[..8].to_vec()
    } else {
        let k = derive_type2auth_key(auth_raw)?;
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

/// Find an interface alternate setting that exposes both requested endpoints.
fn find_interface(
    config: &ConfigDescriptor,
    interface: Option<u8>,
    ep_out: u8,
    ep_in: u8,
) -> Option<(u8, u8, XferKind, XferKind)> {
    for candidate in config.interfaces() {
        for descriptor in candidate.descriptors() {
            if let Some(target) = interface {
                if descriptor.interface_number() != target {
                    continue;
                }
            }

            let mut out_kind = None;
            let mut in_kind = None;
            for endpoint in descriptor.endpoint_descriptors() {
                if endpoint.address() == ep_out {
                    out_kind = Some(xfer_kind(endpoint.transfer_type()));
                }
                if endpoint.address() == ep_in {
                    in_kind = Some(xfer_kind(endpoint.transfer_type()));
                }
            }

            if let (Some(out_kind), Some(in_kind)) = (out_kind, in_kind) {
                return Some((
                    descriptor.interface_number(),
                    descriptor.setting_number(),
                    out_kind,
                    in_kind,
                ));
            }
        }
    }
    None
}

// ==============================================================================
// 4. auRokku USB Device Controller
// ==============================================================================

struct AuRokkuDevice {
    vid: u16,
    pid: u16,
    sn: Option<String>,
    interface_num: Option<u8>,
    interface_alt: u8,
    ep_out: u8,
    ep_in: u8,
    verbose: bool,
    type2: bool,

    context: Option<Context>,
    device: Option<Device<Context>>,
    handle: Option<DeviceHandle<Context>>,
    claimed: bool,
    type2auth_material: Option<Vec<u8>>,
    rx_buffer: Vec<u8>,
    async_events: Vec<Value>,
    exit_result: Option<Value>,
    awaiting_handshake: bool,
    ep_in_kind: XferKind,
    ep_out_kind: XferKind,
}

#[allow(clippy::too_many_arguments)]
impl AuRokkuDevice {
    fn new(
        vid: u16,
        pid: u16,
        sn: Option<String>,
        interface: Option<u8>,
        ep_out: u8,
        ep_in: u8,
        verbose: bool,
        type2: bool,
    ) -> Self {
        Self {
            vid,
            pid,
            sn,
            interface_num: interface,
            interface_alt: 0,
            ep_out,
            ep_in,
            verbose,
            type2,
            context: None,
            device: None,
            handle: None,
            claimed: false,
            type2auth_material: None,
            rx_buffer: Vec::new(),
            async_events: Vec::new(),
            exit_result: None,
            awaiting_handshake: false,
            ep_in_kind: XferKind::Bulk,
            ep_out_kind: XferKind::Bulk,
        }
    }

    fn material(&self) -> AuRokkuResult<&[u8]> {
        self.type2auth_material.as_deref().ok_or_else(|| {
            AuRokkuError::Runtime(
                "Type2Auth material is not initialized (no device SN)".to_string(),
            )
        })
    }

    // --------------------------------------------------------------------------
    // Raw transport
    // --------------------------------------------------------------------------

    fn log(&self, direction: &str, data: &[u8]) {
        if self.verbose {
            eprintln!("{direction}-> {}", hex_spaced(data));
        }
    }

    fn ensure_handle(&mut self) -> AuRokkuResult<()> {
        if self.handle.is_none() {
            let device = self
                .device
                .as_ref()
                .ok_or_else(|| AuRokkuError::Runtime("USB device is not open".to_string()))?;
            let handle = device
                .open()
                .map_err(|e| AuRokkuError::Runtime(format!("USB error: {e}")))?;
            self.handle = Some(handle);
        }
        Ok(())
    }

    fn read_endpoint_in(&self, buf: &mut [u8], timeout: Duration) -> rusb::Result<usize> {
        let handle = self.handle.as_ref().ok_or(rusb::Error::NoDevice)?;
        match self.ep_in_kind {
            XferKind::Interrupt => handle.read_interrupt(self.ep_in, buf, timeout),
            XferKind::Bulk => handle.read_bulk(self.ep_in, buf, timeout),
        }
    }

    fn send(&mut self, data: &[u8]) -> AuRokkuResult<()> {
        self.log("TX", data);

        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| AuRokkuError::Runtime("Device not connected".to_string()))?;

        let timeout = Duration::from_millis(2000);
        let written = match self.ep_out_kind {
            XferKind::Interrupt => handle.write_interrupt(self.ep_out, data, timeout)?,
            XferKind::Bulk => handle.write_bulk(self.ep_out, data, timeout)?,
        };

        if written != data.len() {
            return Err(AuRokkuError::Runtime(format!(
                "Short write: wrote {written}/{} bytes",
                data.len()
            )));
        }
        Ok(())
    }

    fn recv(&mut self, expected_len: usize, timeout: Duration) -> AuRokkuResult<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; 512];

        while self.rx_buffer.len() < expected_len {
            if Instant::now() >= deadline {
                return Err(AuRokkuError::Timeout(format!(
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

    fn read_until(
        &mut self,
        terminator: u8,
        limit: usize,
        timeout: Duration,
    ) -> AuRokkuResult<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let mut data = Vec::new();

        while data.len() < limit {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AuRokkuError::Timeout(
                    "Missing response terminator".to_string(),
                ));
            }
            data.extend(self.recv(1, remaining)?);
            if data.last() == Some(&terminator) {
                return Ok(data);
            }
        }
        Err(AuRokkuError::Runtime(
            "Response exceeds bounded limit".to_string(),
        ))
    }

    // --------------------------------------------------------------------------
    // Connection management
    // --------------------------------------------------------------------------

    /// Find the device and (optionally) derive Type2Auth material from its SN.
    fn open_device(&mut self, require_sn: bool) -> AuRokkuResult<()> {
        if self.device.is_none() {
            let context = Context::new()
                .map_err(|e| AuRokkuError::Runtime(format!("Unable to initialize libusb: {e}")))?;

            let found = {
                let devices = context
                    .devices()
                    .map_err(|e| AuRokkuError::Runtime(format!("USB error: {e}")))?;
                let mut found = None;
                for candidate in devices.iter() {
                    let descriptor = match candidate.device_descriptor() {
                        Ok(descriptor) => descriptor,
                        Err(_) => continue,
                    };
                    if descriptor.vendor_id() == self.vid && descriptor.product_id() == self.pid {
                        found = Some(candidate);
                        break;
                    }
                }
                found
            };

            let device = found.ok_or_else(|| {
                AuRokkuError::Runtime(format!(
                    "Device {:04x}:{:04x} not found",
                    self.vid, self.pid
                ))
            })?;
            self.device = Some(device);
            self.context = Some(context);
        } else if !require_sn || self.type2auth_material.is_some() {
            return Ok(());
        }

        if !require_sn {
            return Ok(());
        }

        let descriptor = self
            .device
            .as_ref()
            .expect("device is open")
            .device_descriptor()
            .map_err(|e| AuRokkuError::Runtime(format!("USB error: {e}")))?;

        let device_sn = if descriptor.serial_number_string_index().is_some() {
            self.ensure_handle()?;
            let handle = self.handle.as_ref().expect("handle ensured");
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
            return Err(AuRokkuError::Runtime(
                "Cannot determine device SN. Please provide --sn <11_chars_or_16_hex> explicitly."
                    .to_string(),
            ));
        };

        let (material, canonical_sn) = parse_sn_to_type2auth(&use_sn)?;
        self.type2auth_material = Some(material);
        self.sn = Some(canonical_sn);

        if self.verbose {
            eprintln!(
                "[INFO] Device SN: {}",
                self.sn.as_deref().unwrap_or_default()
            );
            eprintln!(
                "[INFO] Type2Auth Raw: {}",
                hex_spaced(self.material().unwrap_or_default())
            );
        }
        Ok(())
    }

    /// Open the device, select/claim the interface and prepare the endpoints.
    fn connect(&mut self, require_sn: bool) -> AuRokkuResult<()> {
        self.open_device(require_sn)?;
        self.ensure_handle()?;

        let device = self
            .device
            .clone()
            .ok_or_else(|| AuRokkuError::Runtime("USB device is not open".to_string()))?;
        let config = device
            .active_config_descriptor()
            .map_err(|e| AuRokkuError::Runtime(format!("USB error: {e}")))?;

        let requested_interface = self.interface_num;
        let selected = find_interface(&config, requested_interface, self.ep_out, self.ep_in);

        let (interface_num, alternate_setting, out_kind, in_kind) = match selected {
            Some(selected) => selected,
            None => {
                let message = match requested_interface {
                    Some(number) => format!(
                        "Interface {number} lacks EP OUT 0x{:02X} and EP IN 0x{:02X}; \
                         run --op probe to inspect it",
                        self.ep_out, self.ep_in
                    ),
                    None => format!(
                        "No interface with EP OUT 0x{:02X} and EP IN 0x{:02X}; \
                         run --op probe to list bulk endpoint pairs",
                        self.ep_out, self.ep_in
                    ),
                };
                return Err(AuRokkuError::Runtime(message));
            }
        };

        self.interface_num = Some(interface_num);
        self.interface_alt = alternate_setting;
        self.ep_out_kind = out_kind;
        self.ep_in_kind = in_kind;

        if self.verbose {
            eprintln!(
                "[INFO] Using Interface {interface_num} alt {alternate_setting}, \
                 EP OUT 0x{:02X}, EP IN 0x{:02X}",
                self.ep_out, self.ep_in
            );
        }

        {
            let handle = self
                .handle
                .as_ref()
                .ok_or_else(|| AuRokkuError::Runtime("USB device is not open".to_string()))?;

            if let Ok(true) = handle.kernel_driver_active(interface_num) {
                let _ = handle.detach_kernel_driver(interface_num);
            }

            handle.claim_interface(interface_num)?;
            self.claimed = true;

            if alternate_setting != 0 {
                handle.set_alternate_setting(interface_num, alternate_setting)?;
            }
        }

        self.drain(50);
        Ok(())
    }

    /// Enumerate all USB configurations / interfaces / endpoints and bulk pairs.
    fn inspect_usb(&mut self) -> AuRokkuResult<Value> {
        self.open_device(false)?;

        let device = self
            .device
            .clone()
            .ok_or_else(|| AuRokkuError::Runtime("USB device is not open".to_string()))?;

        let active_value = device
            .active_config_descriptor()
            .ok()
            .map(|config| config.number());

        let num_configurations = device
            .device_descriptor()
            .map_err(|e| AuRokkuError::Runtime(format!("USB error: {e}")))?
            .num_configurations();

        let mut configurations = Vec::new();
        let mut candidates = Vec::new();

        for config_index in 0..num_configurations {
            let config = match device.config_descriptor(config_index) {
                Ok(config) => config,
                Err(_) => continue,
            };

            let config_value = config.number();
            let is_active = active_value == Some(config_value);
            let mut interfaces = Vec::new();

            for candidate in config.interfaces() {
                for descriptor in candidate.descriptors() {
                    let mut endpoints = Vec::new();
                    let mut bulk_in = Vec::new();
                    let mut bulk_out = Vec::new();

                    for endpoint in descriptor.endpoint_descriptors() {
                        let address = endpoint.address();
                        let direction = match endpoint.direction() {
                            Direction::In => "in",
                            Direction::Out => "out",
                        };
                        let transfer_type = match endpoint.transfer_type() {
                            TransferType::Control => "control",
                            TransferType::Isochronous => "isochronous",
                            TransferType::Bulk => "bulk",
                            TransferType::Interrupt => "interrupt",
                        };

                        endpoints.push(json!({
                            "address": format!("0x{address:02x}"),
                            "direction": direction,
                            "type": transfer_type,
                            "max_packet_size": endpoint.max_packet_size(),
                            "interval": endpoint.interval(),
                        }));

                        if endpoint.transfer_type() == TransferType::Bulk {
                            if endpoint.direction() == Direction::In {
                                bulk_in.push(address);
                            } else {
                                bulk_out.push(address);
                            }
                        }
                    }

                    let interface_number = descriptor.interface_number();
                    let alternate_setting = descriptor.setting_number();

                    for ep_out in &bulk_out {
                        for ep_in in &bulk_in {
                            candidates.push(json!({
                                "configuration": config_value,
                                "active": is_active,
                                "interface": interface_number,
                                "alternate_setting": alternate_setting,
                                "ep_out": format!("0x{ep_out:02x}"),
                                "ep_in": format!("0x{ep_in:02x}"),
                            }));
                        }
                    }

                    interfaces.push(json!({
                        "number": interface_number,
                        "alternate_setting": alternate_setting,
                        "class": format!("0x{:02x}", descriptor.class_code()),
                        "subclass": format!("0x{:02x}", descriptor.sub_class_code()),
                        "protocol": format!("0x{:02x}", descriptor.protocol_code()),
                        "endpoints": endpoints,
                    }));
                }
            }

            configurations.push(json!({
                "value": config_value,
                "active": is_active,
                "interfaces": interfaces,
            }));
        }

        Ok(json!({
            "vid": format!("0x{:04x}", self.vid),
            "pid": format!("0x{:04x}", self.pid),
            "active_configuration": active_value,
            "configurations": configurations,
            "bulk_candidates": candidates,
        }))
    }

    fn read_probe_response(&mut self, timeout_sec: f64, limit: usize) -> AuRokkuResult<Vec<u8>> {
        let mut data = std::mem::take(&mut self.rx_buffer);
        let deadline = Instant::now() + Duration::from_secs_f64(timeout_sec.max(0.0));
        let mut buf = [0u8; 512];

        while data.len() < limit && Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let remaining_ms = ((remaining.as_secs_f64() * 1000.0) as u64).clamp(1, 200);
            let want = (limit - data.len()).min(512);

            match self.read_endpoint_in(&mut buf[..want], Duration::from_millis(remaining_ms)) {
                Ok(n) => {
                    if n > 0 {
                        let part = buf[..n].to_vec();
                        self.log("RX", &part);
                        data.extend_from_slice(&part);
                    }
                }
                Err(rusb::Error::Timeout) => {
                    if !data.is_empty() {
                        break;
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }

        Ok(data)
    }

    /// Inspect the USB layout, or actively probe the SERI / SCDP services.
    fn probe(&mut self, kind: ProbeKind, timeout_sec: f64) -> AuRokkuResult<Value> {
        let usb = self.inspect_usb()?;
        let interface_num = self.interface_num;
        let alternate_setting = self.interface_alt;

        let mut result = json!({
            "operation": "probe",
            "probe": kind.as_str(),
            "success": true,
            "selection": {
                "interface": interface_num,
                "alternate_setting": alternate_setting,
                "ep_out": format!("0x{:02x}", self.ep_out),
                "ep_in": format!("0x{:02x}", self.ep_in),
            },
            "usb": usb,
        });

        if kind == ProbeKind::List {
            return Ok(result);
        }

        if !self.claimed {
            return Err(AuRokkuError::Runtime(
                "Active probe requires a claimed interface".to_string(),
            ));
        }

        self.drain(50);

        match kind {
            ProbeKind::Seri => {
                let request = vec![0x1C, 0x05];
                self.send(&request)?;
                let response = self.read_probe_response(timeout_sec, 512)?;
                let matched = response.starts_with(&request);

                result["request_hex"] = json!(hex_spaced(&request));
                result["response_hex"] = json!(hex_spaced(&response));
                result["responsive"] = json!(!response.is_empty());
                result["matched"] = json!(matched);
                result["success"] = json!(matched);

                if matched && self.vid != 0x0FCE {
                    self.send(&[0xD8])?;
                    result["cleanup_hex"] = json!("d8");
                }
            }
            ProbeKind::Scdp => {
                let request = vec![0x8E];
                self.send(&request)?;
                let response = self.read_probe_response(timeout_sec, 512)?;

                result["request_hex"] = json!(hex_spaced(&request));
                result["response_hex"] = json!(hex_spaced(&response));
                result["responsive"] = json!(!response.is_empty());
                result["response_length"] = json!(response.len());
                result["success"] = json!(!response.is_empty());
            }
            ProbeKind::List => unreachable!("handled above"),
        }

        Ok(result)
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
        self.device = None;
        self.context = None;
    }

    // --------------------------------------------------------------------------
    // Session & Authentication
    // --------------------------------------------------------------------------

    /// Perform challenge-response authentication for a service.
    fn authenticate_step(&mut self, service_name: &str) -> AuRokkuResult<String> {
        self.send(&[0x52])?;

        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        let mut resp = self.recv(17, DEFAULT_TIMEOUT)?;
        let mut extra_echoes = 0;

        while service_name == "uimlk" && resp.first() == Some(&0xE7) {
            if extra_echoes >= 4 {
                return Err(AuRokkuError::Runtime(
                    "Too many E7 acknowledgements before UIMLK challenge".to_string(),
                ));
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AuRokkuError::Timeout(
                    "Timeout completing UIMLK challenge after E7 acknowledgement".to_string(),
                ));
            }

            extra_echoes += 1;
            self.async_events.push(json!({
                "opcode": "e7",
                "meaning": "extra_uimlk_ack_before_challenge"
            }));

            let tail = self.recv(1, remaining)?;
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
        self.send(&auth_packet)?;

        let auth_resp = self.recv(2, DEFAULT_TIMEOUT)?;
        if auth_resp != [0x53, 0x01] {
            return Err(AuRokkuError::Runtime(format!(
                "Authentication failed for {service_name}: {}",
                hex_spaced(&auth_resp)
            )));
        }

        Ok(hex::encode(&challenge))
    }

    /// Establish base session and, when `--type2` is set, authenticate base.
    fn enter_base(&mut self) -> AuRokkuResult<()> {
        self.drain(50);

        if self.vid != 0x0FCE && self.send(&[0xD8]).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
            self.drain(50);
        }

        self.send(&[0x1C, 0x05])?;
        self.awaiting_handshake = true;
        let echo_result = self.recv(2, DEFAULT_TIMEOUT);
        self.awaiting_handshake = false;
        let echo = echo_result?;

        if echo != [0x1C, 0x05] {
            return Err(AuRokkuError::Runtime(format!(
                "Base handshake echo mismatch: {}",
                hex_spaced(&echo)
            )));
        }

        if self.type2 {
            self.authenticate_step("base")?;
        } else if self.verbose {
            eprintln!("[INFO] Default mode: skipping base authentication");
        }
        Ok(())
    }

    /// Establish session, authenticate base, switch to UIMLK (0xE7) and authenticate.
    fn enter_uimlk(&mut self) -> AuRokkuResult<()> {
        self.enter_base()?;

        self.send(&[0xE7])?;
        let switch_echo = self.recv(1, DEFAULT_TIMEOUT)?;
        if switch_echo != [0xE7] {
            return Err(AuRokkuError::Runtime(format!(
                "Service switch to UIMLK failed: {}",
                hex_spaced(&switch_echo)
            )));
        }

        self.authenticate_step("uimlk")?;
        Ok(())
    }

    /// Send base / mask2 exit command (0xD8) to restore the phone keypad / normal UI.
    fn exit_base(&mut self) {
        if self.send(&[0xD8]).is_ok() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Leave base mode (non-0FCE devices only); returns the command used.
    fn exit_mode(&mut self) -> Option<Vec<u8>> {
        if self.vid != 0x0FCE {
            self.exit_base();
            return Some(vec![0xD8]);
        }
        None
    }

    /// Send 0x56 exit command.
    fn exit_uimlk(&mut self) -> AuRokkuResult<Vec<u8>> {
        self.send(&[0x56])?;
        let resp = self.recv(1, DEFAULT_TIMEOUT)?;
        if resp != [0x56] {
            return Err(AuRokkuError::Runtime(format!(
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
    fn read_state(&mut self) -> AuRokkuResult<Value> {
        self.send(&[0x54])?;
        let resp = self.recv(10, DEFAULT_TIMEOUT)?;

        if resp.len() != 10 || resp[0] != 0x74 || (resp[1] != 0 && resp[1] != 1) {
            return Err(AuRokkuError::Runtime(format!(
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
    fn read_uimlk(&mut self) -> AuRokkuResult<Value> {
        self.enter_uimlk()?;
        let result = self.read_uimlk_inner();
        self.finish_uimlk();
        result
    }

    fn read_uimlk_inner(&mut self) -> AuRokkuResult<Value> {
        let state = self.read_state()?;
        Ok(prepend_fields(
            state,
            vec![("operation", json!("readuimlk")), ("success", json!(true))],
        ))
    }

    /// Perform a UIMLK mutating command with readback verification.
    fn mutate(
        &mut self,
        operation: &str,
        packet: &[u8],
        expected_reply: &[u8],
        expected_value: &str,
        control: Option<u8>,
    ) -> AuRokkuResult<Value> {
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
    ) -> AuRokkuResult<Value> {
        let mut result = json!({
            "operation": operation,
            "success": false,
            "persistence_verified": false,
        });

        let before = self.read_state()?;
        result["before"] = before.clone();

        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.send(packet)?;
            self.recv(2, DEFAULT_TIMEOUT)
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

        match self.read_state() {
            Ok(after) => {
                let after_value = after
                    .get("lock_value")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let after_control =
                    after.get("control").and_then(Value::as_u64).unwrap_or(255) as u8;
                let expected_control = control.unwrap_or_else(|| {
                    before.get("control").and_then(Value::as_u64).unwrap_or(0) as u8
                });

                result["after"] = after;
                result["verified"] =
                    json!(after_value == expected_value && after_control == expected_control);
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
    fn write_uimlk(&mut self, lock_val_hex: &str, control: u8) -> AuRokkuResult<Value> {
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

    /// Clear/Delete UIM lock to FFFFFFFF (SIM unlock).
    fn clear_uimlk(&mut self) -> AuRokkuResult<Value> {
        self.mutate("clearlk", &[0x57], &[0x57, 0x01], "ffffffff", None)
    }

    /// Read the SIM ICCID record (opcode 0x59) within a UIMLK session.
    fn read_iccid(&mut self) -> AuRokkuResult<Value> {
        self.enter_uimlk()?;
        let result = self.read_iccid_inner();
        self.finish_uimlk();
        result
    }

    fn read_iccid_inner(&mut self) -> AuRokkuResult<Value> {
        self.send(&[0x59])?;
        let resp = self.recv(21, DEFAULT_TIMEOUT)?;

        if resp.len() != 21 || resp[0] != 0x59 {
            return Err(AuRokkuError::Runtime(format!(
                "Invalid record59 reply: {}",
                hex_spaced(&resp)
            )));
        }

        Ok(json!({
            "operation": "readiccid",
            "success": true,
            "raw_hex": hex_spaced(&resp),
            "decoded_hex": hex_spaced(&decode_nibbles(&resp[1..])?),
        }))
    }

    /// Internal query of record 59 within an active UIMLK session.
    fn query_record59(&mut self) -> AuRokkuResult<Value> {
        self.send(&[0x59])?;
        let resp = self.recv(21, DEFAULT_TIMEOUT)?;
        if resp.is_empty() || resp[0] != 0x59 {
            return Err(AuRokkuError::Runtime("Invalid record59 reply".to_string()));
        }

        Ok(json!({
            "decoded_hex": hex_spaced(&decode_nibbles(&resp[1..])?),
            "raw_hex": hex_spaced(&resp),
        }))
    }

    // --------------------------------------------------------------------------
    // Phone Lock Code (PIN) Operations (NV 82)
    // --------------------------------------------------------------------------

    /// Read phone Lock Code / PIN (NV 82) via SERI 0xDC 0x00.
    fn read_lockcode(&mut self, exit_after: bool) -> AuRokkuResult<Value> {
        let result = self.read_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn read_lockcode_inner(&mut self) -> AuRokkuResult<Value> {
        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.send(&[0xDC, 0x00])?;
            self.read_until(0xAA, 256, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.send(&[0xDC, 0x00])?;
                self.read_until(0xAA, 256, DEFAULT_TIMEOUT)?
            }
        };

        let index =
            find_subslice(&reply, &[0xDC, 0x00]).ok_or_else(|| invalid_lockcode_reply(&reply))?;
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
    fn reset_lockcode(&mut self, exit_after: bool) -> AuRokkuResult<Value> {
        let result = self.reset_lockcode_inner();
        if exit_after {
            self.exit_mode();
        }
        result
    }

    fn reset_lockcode_inner(&mut self) -> AuRokkuResult<Value> {
        let reply_result: AuRokkuResult<Vec<u8>> = (|| {
            self.send(&[0xDC, 0x01])?;
            self.recv(2, Duration::from_secs(1))
        })();

        let reply = match reply_result {
            Ok(reply) => reply,
            Err(_) => {
                self.enter_base()?;
                self.send(&[0xDC, 0x01])?;
                self.recv(2, DEFAULT_TIMEOUT)?
            }
        };

        if reply != [0xDC, 0x01] {
            return Err(AuRokkuError::Runtime(format!(
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
    fn write_maintenance_bit(&mut self, control: u8) -> AuRokkuResult<Value> {
        if control > 1 {
            return Err(AuRokkuError::Value(
                "maintenance_bit requires --control 0 or 1".to_string(),
            ));
        }

        self.enter_base()?;
        self.send(&[0xE1])?;
        if self.recv(1, DEFAULT_TIMEOUT)? != [0xE1] {
            return Err(AuRokkuError::Runtime(
                "Maintenance service (0xE1) entry failed".to_string(),
            ));
        }
        self.authenticate_step("4000")?;

        let packet = [0x72, control];
        let expected = [0x72, 0x01];

        let mut result = json!({
            "operation": "maintenance_bit",
            "success": false,
            "maintenance_bit": control,
        });

        self.send(&packet)?;
        let reply = self.recv(2, DEFAULT_TIMEOUT)?;
        let accepted = reply == expected;

        result["reply"] = json!(hex_spaced(&reply));
        result["command_accepted"] = json!(accepted);
        result["success"] = json!(accepted);
        Ok(result)
    }

    // --------------------------------------------------------------------------
    // Session Queries & Status
    // --------------------------------------------------------------------------

    /// Query basic parameters in base mode.
    fn query_basic(&mut self, name: &str) -> AuRokkuResult<Value> {
        match name {
            "version" => {
                self.send(&[0x1B])?;
                let reply = self.recv(5, DEFAULT_TIMEOUT)?;
                if reply.is_empty() || reply[0] != 0x1B {
                    return Err(AuRokkuError::Runtime("Invalid version reply".to_string()));
                }
                Ok(json!({ "raw_hex": hex_spaced(&reply) }))
            }
            "lockcode" => {
                self.send(&[0xDC, 0x00])?;
                let reply = self.read_until(0xAA, 256, DEFAULT_TIMEOUT)?;
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
            other => Err(AuRokkuError::Value(format!("Unknown query: {other}"))),
        }
    }

    /// Execute multiple queries in a single session without intermediate exits.
    fn batch_read(&mut self, profile: &str) -> AuRokkuResult<Value> {
        let mut report = json!({
            "operation": "batch",
            "profile": profile,
            "success": false,
            "results": [],
            "automatic_exit": false,
        });

        let outcome: AuRokkuResult<()> = (|| {
            self.enter_base()?;
            let mut results: Vec<Value> = Vec::new();

            match profile {
                "basic" => {
                    results.push(with_query("version", self.query_basic("version")?));
                    results.push(with_query("lockcode", self.query_basic("lockcode")?));
                }
                "uimlk" => {
                    self.send(&[0xE7])?;
                    if self.recv(1, DEFAULT_TIMEOUT)? != [0xE7] {
                        return Err(AuRokkuError::Runtime("UIM entry failed".to_string()));
                    }
                    self.authenticate_step("uimlk")?;
                    results.push(with_query("state", self.read_state()?));
                    results.push(with_query("record59", self.query_record59()?));
                }
                other => {
                    return Err(AuRokkuError::Value(format!(
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
    fn read_status(&mut self) -> AuRokkuResult<Value> {
        let mut report = json!({
            "operation": "status",
            "sn": self.sn.clone(),
            "success": false,
        });
        let mut in_uimlk = false;

        let outcome: AuRokkuResult<()> = (|| {
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
            if self.recv(1, DEFAULT_TIMEOUT)? != [0xE7] {
                return Err(AuRokkuError::Runtime("Switch to UIMLK failed".to_string()));
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

    /// Send exit request (0xD8 for base/PIN, 0x56 for UIMLK) to return to normal mode.
    fn exit_service_only(&mut self, op: &str) -> AuRokkuResult<Value> {
        if op == "exituimlk" {
            self.enter_uimlk()?;
            let reply = self.exit_uimlk()?;
            Ok(json!({
                "operation": "exituimlk",
                "success": reply == [0x56],
                "method": "handshake_and_exit",
                "reply": hex::encode(&reply),
            }))
        } else {
            self.exit_base();
            Ok(json!({
                "operation": "exit",
                "success": true,
                "method": "0xd8",
            }))
        }
    }
}

fn invalid_lockcode_reply(reply: &[u8]) -> AuRokkuError {
    AuRokkuError::Runtime(format!("Invalid lockcode reply: {}", hex_spaced(reply)))
}

fn prepend_fields(value: Value, fields: Vec<(&str, Value)>) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, field) in fields {
                out.insert(key.to_string(), field);
            }
            for (key, field) in map {
                out.insert(key, field);
            }
            Value::Object(out)
        }
        other => other,
    }
}

fn with_query(name: &str, value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            out.insert("query".to_string(), json!(name));
            for (key, field) in map {
                out.insert(key, field);
            }
            Value::Object(out)
        }
        other => json!({ "query": name, "value": other }),
    }
}

// ==============================================================================
// 5. CLI & Human-Friendly Presentation
// ==============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Op {
    #[value(name = "status")]
    Status,
    #[value(name = "readuimlk")]
    ReadUimlk,
    #[value(name = "clearlk")]
    Clearlk,
    #[value(name = "writelk")]
    Writelk,
    #[value(name = "readiccid")]
    Readiccid,
    #[value(name = "readpin")]
    Readpin,
    #[value(name = "resetpin")]
    Resetpin,
    #[value(name = "maintenance_bit")]
    MaintenanceBit,
    #[value(name = "batch")]
    Batch,
    #[value(name = "probe")]
    Probe,
    #[value(name = "exituimlk")]
    Exituimlk,
    #[value(name = "exit")]
    Exit,
}

impl Op {
    fn as_str(self) -> &'static str {
        match self {
            Op::Status => "status",
            Op::ReadUimlk => "readuimlk",
            Op::Clearlk => "clearlk",
            Op::Writelk => "writelk",
            Op::Readiccid => "readiccid",
            Op::Readpin => "readpin",
            Op::Resetpin => "resetpin",
            Op::MaintenanceBit => "maintenance_bit",
            Op::Batch => "batch",
            Op::Probe => "probe",
            Op::Exituimlk => "exituimlk",
            Op::Exit => "exit",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum ProbeKind {
    #[value(name = "list")]
    List,
    #[value(name = "seri")]
    Seri,
    #[value(name = "scdp")]
    Scdp,
}

impl ProbeKind {
    fn as_str(self) -> &'static str {
        match self {
            ProbeKind::List => "list",
            ProbeKind::Seri => "seri",
            ProbeKind::Scdp => "scdp",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Profile {
    #[value(name = "basic")]
    Basic,
    #[value(name = "uimlk")]
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

#[derive(Debug, Parser)]
#[command(
    name = "auroku",
    about = "auRokku device service tool",
    after_help = EXAMPLES
)]
struct Cli {
    /// Operation to perform on the device
    #[arg(long, value_enum, required = true)]
    op: Op,

    /// USB vendor ID in hex
    #[arg(long, value_parser = parse_hex_u16, required = true)]
    vid: u16,

    /// USB product ID in hex
    #[arg(long, value_parser = parse_hex_u16, required = true)]
    pid: u16,

    /// 11-character serial number or 16-hex Type2Auth value (auto-detected if omitted)
    #[arg(long)]
    sn: Option<String>,

    /// 4-byte hex value for writelk (e.g. 80e48ed2)
    #[arg(long = "val")]
    val: Option<String>,

    /// Control bit (0 or 1) for writelk / maintenance_bit
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=1))]
    control: Option<u8>,

    /// USB interface number (auto-detected if omitted)
    #[arg(long)]
    interface: Option<u8>,

    /// USB OUT endpoint in hex
    #[arg(long = "ep-out", value_parser = parse_usb_endpoint, default_value = "04")]
    ep_out: u8,

    /// USB IN endpoint in hex
    #[arg(long = "ep-in", value_parser = parse_usb_endpoint, default_value = "84")]
    ep_in: u8,

    /// Probe USB layout, SERI, or SCDP
    #[arg(long = "probe-kind", value_enum, default_value = "list")]
    probe_kind: ProbeKind,

    /// Active probe timeout in seconds
    #[arg(long = "probe-timeout", default_value_t = 1.0)]
    probe_timeout: f64,

    /// Batch profile
    #[arg(long, value_enum, default_value = "basic")]
    profile: Profile,

    /// Show USB traffic
    #[arg(long)]
    verbose: bool,

    /// Enable 52/53 authentication
    #[arg(long)]
    type2: bool,

    /// Output JSON
    #[arg(long)]
    json: bool,
}

fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().map(|n| n != 0.0).unwrap_or(true),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Python-like `str(value)` for scalars used in reports.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(value) => py_bool(*value).to_string(),
        other => other.to_string(),
    }
}

/// `result.get(key, "")` in Python terms.
fn str_field(value: &Value, key: &str) -> String {
    value.get(key).map(py_str).unwrap_or_default()
}

/// `result.get(key, default)` in Python terms.
fn str_field_or(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .map(py_str)
        .unwrap_or_else(|| default.to_string())
}

/// `a or b or c` semantics across JSON fields.
fn first_truthy(value: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(field) = value.get(*key) {
            if is_truthy(field) {
                return py_str(field);
            }
        }
    }
    "None".to_string()
}

/// Print a formatted summary of the operation for human reading.
fn print_human_report(op: &str, result: &Value) {
    println!("{}", "=".repeat(60));
    println!("  Operation: {op}");
    println!("{}", "=".repeat(60));

    let success = result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    println!(
        "  Result Status : {}",
        if success { "SUCCESS" } else { "FAILED" }
    );

    match op {
        "readuimlk" => {
            let value = str_field(result, "lock_value");
            if value == "ffffffff" {
                println!("  UIM Lock      : UNLOCKED / CLEARED (ffffffff)");
            } else {
                println!("  UIM Lock      : LOCKED (Bound Card: {value})");
            }
            println!("  Control Bit   : {}", str_field(result, "control"));
            println!("  Raw Hex       : {}", str_field(result, "raw_hex"));
        }
        "clearlk" => {
            let empty = json!({});
            let before = result.get("before").unwrap_or(&empty);
            let after = result.get("after").unwrap_or(&empty);
            println!(
                "  Before Lock   : {} (control: {})",
                str_field(before, "lock_value"),
                str_field(before, "control")
            );
            println!(
                "  After Lock    : {} (control: {})",
                str_field(after, "lock_value"),
                str_field(after, "control")
            );
            println!(
                "  Verified      : {}",
                py_bool(result.get("verified").map(is_truthy).unwrap_or(false))
            );
        }
        "writelk" => {
            let empty = json!({});
            let before = result.get("before").unwrap_or(&empty);
            let after = result.get("after").unwrap_or(&empty);
            println!("  Before Lock   : {}", str_field(before, "lock_value"));
            println!(
                "  Written Lock  : {} (control: {})",
                str_field(after, "lock_value"),
                str_field(after, "control")
            );
            println!(
                "  Verified      : {}",
                py_bool(result.get("verified").map(is_truthy).unwrap_or(false))
            );
        }
        "readiccid" => {
            println!("  Decoded ICCID : {}", str_field(result, "decoded_hex"));
            println!("  Raw Response  : {}", str_field(result, "raw_hex"));
        }
        "readpin" => {
            println!("  Phone PIN     : {}", str_field(result, "lockcode"));
        }
        "resetpin" => {
            println!("  New PIN       : {}", str_field(result, "lockcode"));
            println!(
                "  Verified 1234 : {}",
                py_bool(result.get("verified").map(is_truthy).unwrap_or(false))
            );
        }
        "maintenance_bit" => {
            println!(
                "  Requested     : maintenance_bit = {}",
                str_field_or(result, "maintenance_bit", "None")
            );
            println!(
                "  Accepted      : {}",
                py_bool(
                    result
                        .get("command_accepted")
                        .map(is_truthy)
                        .unwrap_or(false)
                )
            );
        }
        "status" => {
            println!("  Device SN     : {}", str_field_or(result, "sn", "N/A"));
            println!(
                "  Base Version  : {}",
                str_field_or(result, "version", "N/A")
            );
            println!(
                "  Phone PIN     : {}",
                str_field_or(result, "lockcode", "N/A")
            );

            if let Some(uim) = result.get("uim_lock").filter(|value| is_truthy(value)) {
                let unlocked = uim.get("unlocked").map(is_truthy).unwrap_or(false);
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

            if result.get("iccid_record").map(is_truthy).unwrap_or(false) {
                println!("  SIM ICCID     : {}", str_field(result, "iccid_record"));
            }
        }
        "batch" => {
            println!("  Profile       : {}", str_field(result, "profile"));
            if let Some(items) = result.get("results").and_then(Value::as_array) {
                for item in items {
                    let name = str_field(item, "query");
                    let display = first_truthy(item, &["decoded", "lock_value", "raw_hex"]);
                    println!("    - {name:<12}: {display}");
                }
            }
        }
        "probe" => {
            let empty = json!({});
            let usb_info = result.get("usb").unwrap_or(&empty);
            println!(
                "  Device        : {}:{}",
                str_field(usb_info, "vid"),
                str_field(usb_info, "pid")
            );
            println!("  Probe Type    : {}", str_field(result, "probe"));

            let probe_kind = str_field(result, "probe");
            if probe_kind != "list" {
                let selection = result.get("selection").unwrap_or(&empty);
                println!(
                    "  Selected      : if {} alt {} OUT {} IN {}",
                    str_field(selection, "interface"),
                    str_field(selection, "alternate_setting"),
                    str_field(selection, "ep_out"),
                    str_field(selection, "ep_in")
                );
            }

            let candidates = usb_info
                .get("bulk_candidates")
                .and_then(Value::as_array)
                .filter(|items| !items.is_empty());

            if let Some(candidates) = candidates {
                println!("  Bulk Pairs    :");
                for item in candidates {
                    let active = if item.get("active").map(is_truthy).unwrap_or(false) {
                        " active"
                    } else {
                        ""
                    };
                    println!(
                        "    cfg {} if {} alt {} OUT {} IN {}{}",
                        str_field(item, "configuration"),
                        str_field(item, "interface"),
                        str_field(item, "alternate_setting"),
                        str_field(item, "ep_out"),
                        str_field(item, "ep_in"),
                        active
                    );
                }
            } else {
                println!("  Bulk Pairs    : none");
            }

            if result.get("request_hex").map(is_truthy).unwrap_or(false) {
                println!("  Request       : {}", str_field(result, "request_hex"));
                let response = str_field(result, "response_hex");
                let response = if response.is_empty() {
                    "<timeout>".to_string()
                } else {
                    response
                };
                println!("  Response      : {response}");
                println!(
                    "  Responsive    : {}",
                    py_bool(result.get("responsive").map(is_truthy).unwrap_or(false))
                );
            }

            if probe_kind == "seri" {
                println!(
                    "  SERI Echo     : {}",
                    py_bool(result.get("matched").map(is_truthy).unwrap_or(false))
                );
            }
        }
        "exituimlk" | "exit" => {
            println!("  Method        : {}", str_field(result, "method"));
        }
        _ => {}
    }

    if let Some(exit_info) = result.get("exit").filter(|value| is_truthy(value)) {
        if exit_info.is_object() {
            if exit_info
                .get("acknowledged")
                .map(is_truthy)
                .unwrap_or(false)
            {
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
    }

    println!("{}", "=".repeat(60));
}

// ==============================================================================
// 6. Main
// ==============================================================================

fn dispatch(device: &mut AuRokkuDevice, cli: &Cli) -> AuRokkuResult<Value> {
    match cli.op {
        Op::Status => device.read_status(),
        Op::ReadUimlk => device.read_uimlk(),
        Op::Clearlk => device.clear_uimlk(),
        Op::Writelk => {
            let value = cli.val.clone().unwrap_or_default();
            device.write_uimlk(&value, cli.control.unwrap_or(1))
        }
        Op::Readiccid => device.read_iccid(),
        Op::Readpin => device.read_lockcode(true),
        Op::Resetpin => device.reset_lockcode(true),
        Op::MaintenanceBit => {
            device.write_maintenance_bit(cli.control.expect("--control validated in main"))
        }
        Op::Batch => device.batch_read(cli.profile.as_str()),
        Op::Exituimlk => device.exit_service_only("exituimlk"),
        Op::Exit => device.exit_service_only("exit"),
        Op::Probe => unreachable!("probe is handled by run_session"),
    }
}

fn run_session(device: &mut AuRokkuDevice, cli: &Cli) -> AuRokkuResult<Value> {
    let mut result = if cli.op == Op::Probe {
        device.open_device(false)?;
        if cli.probe_kind != ProbeKind::List {
            device.connect(false)?;
        }
        device.probe(cli.probe_kind, cli.probe_timeout)?
    } else {
        device.connect(true)?;
        dispatch(device, cli)?
    };

    result["exit"] = device.exit_result.clone().unwrap_or(Value::Null);
    result["async_events"] = Value::Array(device.async_events.clone());
    Ok(result)
}

fn main() {
    let cli = Cli::parse();

    if cli.op == Op::Writelk && cli.val.is_none() {
        eprintln!("Error: --op writelk requires --val <8_hex_chars> (e.g. --val 80e48ed2)");
        std::process::exit(1);
    }
    if cli.op == Op::MaintenanceBit && cli.control.is_none() {
        eprintln!("Error: --op maintenance_bit requires --control 0 or 1");
        std::process::exit(1);
    }
    if cli.ep_out & 0x80 != 0 {
        eprintln!("Error: --ep-out must be an OUT endpoint (bit 7 clear, e.g. 04)");
        std::process::exit(1);
    }
    if cli.ep_in & 0x80 == 0 {
        eprintln!("Error: --ep-in must be an IN endpoint (bit 7 set, e.g. 84)");
        std::process::exit(1);
    }
    if cli.probe_timeout.is_nan() || cli.probe_timeout <= 0.0 {
        eprintln!("Error: --probe-timeout must be greater than zero");
        std::process::exit(1);
    }

    let mut device = AuRokkuDevice::new(
        cli.vid,
        cli.pid,
        cli.sn.clone(),
        cli.interface,
        cli.ep_out,
        cli.ep_in,
        cli.verbose,
        cli.type2,
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
                print_human_report(cli.op.as_str(), &result);
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
    fn parse_sn_11_char() {
        let (raw, name) = parse_sn_to_type2auth("SKYKA077394").unwrap();
        assert_eq!(hex::encode(&raw), "947307414b594b53");
        assert_eq!(name, "SKYKA077394");
    }

    #[test]
    fn parse_sn_16_hex_keeps_raw_material() {
        let (raw, name) = parse_sn_to_type2auth("947307414B594B53").unwrap();
        assert_eq!(hex::encode(&raw), "947307414b594b53");
        assert_eq!(name, "947307414B594B53");
    }

    #[test]
    fn parse_sn_rejects_garbage() {
        // Python returns empty material for an empty SN; key derivation then fails.
        let (empty, _) = parse_sn_to_type2auth("").unwrap();
        assert!(derive_type2auth_key(&empty).is_err());

        assert!(parse_sn_to_type2auth("SKYKA07739").is_err());
        assert!(parse_sn_to_type2auth("ZZZZZZZZZZZZZZZZ").is_err());
    }

    #[test]
    fn derive_key_vector() {
        let (raw, _) = parse_sn_to_type2auth("SKYKA077394").unwrap();
        assert_eq!(
            hex::encode(derive_type2auth_key(&raw).unwrap()),
            "9a0794d253ce824b"
        );
    }

    #[test]
    fn auth_request_vectors() {
        let (raw, _) = parse_sn_to_type2auth("SKYKA077394").unwrap();
        let challenge = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

        assert_eq!(
            hex::encode(compute_type2auth_request("base", &challenge, &raw).unwrap()),
            "53050602070601030d0b0c000d0b070f01"
        );
        assert_eq!(
            hex::encode(compute_type2auth_request("uimlk", &challenge, &raw).unwrap()),
            "53050602070601030d0b0c000d0b070f01"
        );
        assert_eq!(
            hex::encode(compute_type2auth_request("4000", &challenge, &raw).unwrap()),
            "5304030500090706010e0e0f01070b0d05"
        );

        for service in ["base", "uimlk", "2000", "4000"] {
            let packet = compute_type2auth_request(service, &challenge, &raw).unwrap();
            assert_eq!(packet.len(), 17);
            assert_eq!(packet[0], 0x53);
            assert!(packet[1..].iter().all(|&b| b <= 0x0F));
        }
    }

    #[test]
    fn auth_request_rejects_bad_service_and_challenge() {
        let (raw, _) = parse_sn_to_type2auth("SKYKA077394").unwrap();
        assert!(compute_type2auth_request("bogus", &[0u8; 8], &raw).is_err());
        assert!(compute_type2auth_request("base", &[0u8; 7], &raw).is_err());
    }

    #[test]
    fn lockcode_digits_decode() {
        // 1 2 3 4 * # 0 P -
        let raw = [0x91, 0x92, 0x93, 0x94, 0x9B, 0x9C, 0x9A, 0x9E, 0x9F];
        assert_eq!(decode_lockcode_digits(&raw), "1234*#0P-");
        assert_eq!(decode_lockcode_digits(&[0x80]), "[80]");
    }

    #[test]
    fn usb_endpoint_parsing() {
        assert_eq!(parse_usb_endpoint("04").unwrap(), 0x04);
        assert_eq!(parse_usb_endpoint("0x84").unwrap(), 0x84);
        assert_eq!(parse_usb_endpoint("FF").unwrap(), 0xFF);
        assert!(parse_usb_endpoint("100").is_err());
        assert!(parse_usb_endpoint("no").is_err());
    }

    #[test]
    fn find_subslice_works() {
        assert_eq!(
            find_subslice(&[0x00, 0xDC, 0x00, 0xAA], &[0xDC, 0x00]),
            Some(1)
        );
        assert_eq!(find_subslice(&[0x00, 0xAA], &[0xDC]), None);
    }

    #[test]
    fn hex_spaced_format() {
        assert_eq!(hex_spaced(&[0x74, 0x01, 0xFF]), "74 01 ff");
        assert_eq!(hex_spaced(&[]), "");
    }
}
