//! Protocol codec helpers.
//!
//! Copied from the desktop `src/lib.rs` §1 (lines 123-314). Every function is
//! pure and allocation-only, so it works unchanged in `no_std`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{AuRokkuError, AuRokkuResult};

/// Split each byte into two nibble bytes: high nibble and low nibble (00..0F).
///
/// Original: `encode_nibbles` (`src/lib.rs:123-130`).
pub fn encode_nibbles(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        out.push(b >> 4);
        out.push(b & 0x0F);
    }
    out
}

/// Combine pairs of nibble bytes (00..0F) into original bytes.
///
/// Original: `decode_nibbles` (`src/lib.rs:133-145`).
pub fn decode_nibbles(data: &[u8]) -> AuRokkuResult<Vec<u8>> {
    if !data.len().is_multiple_of(2) || data.iter().any(|&b| b > 0x0F) {
        return Err(AuRokkuError::Value(
            "Expected an even number of binary nibble bytes (00..0F)".to_string(),
        ));
    }
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| (chunk[0] << 4) | chunk[1])
        .collect())
}

/// Decode SERI 0xDC lockcode response bytes into an ASCII string.
///
/// Original: `decode_lockcode_digits` (`src/lib.rs:148-162`).
pub fn decode_lockcode_digits(data: &[u8]) -> String {
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

/// argparse-style hex USB endpoint parser (CLI helper, kept for parity/tests).
///
/// Original: `parse_usb_endpoint` (`src/lib.rs:195-207`).
pub fn parse_usb_endpoint(value: &str) -> Result<u8, String> {
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

/// Format bytes as lowercase, space-separated hex (`74 01 ff`).
///
/// Original: `hex_spaced` (`src/lib.rs:302-307`).
pub fn hex_spaced(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Find the first occurrence of `needle` inside `haystack`.
///
/// Original: `find_subslice` (`src/lib.rs:309-314`).
pub fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}
