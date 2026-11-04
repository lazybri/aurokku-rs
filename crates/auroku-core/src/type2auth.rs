//! Type2Auth mathematics (`0x52`/`0x53` challenge-response).
//!
//! Copied from the desktop `src/lib.rs` §1 (lines 170-296). The whole scheme is
//! plain MD5 plus byte permutation / bit reversal: no DES/AES, no RNG and no
//! clock input, so it ports to `no_std` unchanged. Test vectors from the
//! desktop test suite are re-used verbatim in this crate's tests.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use md5::{Digest, Md5};

use crate::codec::encode_nibbles;
use crate::error::{AuRokkuError, AuRokkuResult};

/// Parse a serial number into the 8 raw Type2Auth bytes plus the canonical name.
///
/// A 16-character SN is treated as hex material; anything else uses the first
/// `min(5, len)` characters as ASCII and the remainder as hex, then reverses the
/// byte string - exactly like the original implementation.
///
/// Original: `parse_sn_to_type2auth` (`src/lib.rs:170-192`).
pub fn parse_sn_to_type2auth(sn_str: &str) -> AuRokkuResult<(Vec<u8>, String)> {
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

/// Reverse the 8 bits of a single byte.
///
/// Original: `bitrev8` (`src/lib.rs:223-225`).
pub fn bitrev8(b: u8) -> u8 {
    b.reverse_bits()
}

/// Derive key material K from the 8 raw Type2Auth bytes.
///
/// Original: `derive_type2auth_key` (`src/lib.rs:228-246`).
pub fn derive_type2auth_key(auth_raw: &[u8]) -> AuRokkuResult<[u8; 8]> {
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

/// MD5 over a sequence of chunks.
///
/// Original: `md5_digest` (`src/lib.rs:248-257`).
pub fn md5_digest(chunks: &[&[u8]]) -> [u8; 16] {
    let mut hasher = Md5::new();
    for chunk in chunks {
        hasher.update(chunk);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

/// Compute the SERI `0x53` authentication response packet.
///
/// Original: `compute_type2auth_request` (`src/lib.rs:260-296`).
pub fn compute_type2auth_request(
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
