# Upstream protocol reference / 上游协议实现参考

This directory vendors the **upstream Python protocol implementation** that the
Rust port is based on. It is kept as a *context premise* for protocol work and
for comparing the Rust implementation with the original behaviour. The copy is
**not compiled, linked or shipped** in any auRokku release artifact.

- Upstream repository: <https://github.com/saga0324/auRokku>
- Pinned commit: `3b1eccf01eb8429b98aa43696fb47673a21241bd`
  (2026-10-01, "fix: Remove SN Check and Probe Settings")
- `auRokku.py` blob: `f3a9b36d869b70bada7471f93e69cd659baa4cac`
- `auRokku.py` SHA256:
  `2df3c9e0b12304c6b497f7beb3904551c441514e799cf8d77c27a1da205041de`
- Licence: GNU General Public License v3.0 (see `LICENSE` in this directory;
  the upstream project and auRokku are both GPL-3.0).

## Why it is here

The Rust `main` branch is a port of this file. Keeping the pinned source next to
the Rust tree lets reviewers and future sessions check protocol constants,
authentication mathematics, packet framing and SN handling against the original
instead of relying on memory. The Rust canonical implementation is located in
`src/lib.rs` (protocol codec, Type2Auth and SN parsing).

## Protocol facts that constrain the Rust implementation

- SERI frames are bulk/interrupt transfers; authentication uses `0x52`
  (challenge) and `0x53` (proof) with MD5-based key derivation
  (`derive_type2auth_key` / `compute_type2auth_request`).
- Service entry (`0x1C 0x05`), UIMLK switch (`0xE7`) and exit packets
  (`0xD8` / `0x56`) are unchanged by the Rust port.
- Upstream `parse_sn_to_type2auth` has **no** fallback parser: a 16-hex SN is
  used verbatim, otherwise `forward = upper(s[:5]).encode() + bytes.fromhex(s[5:])`
  is stored reversed. The Rust port keeps exactly that canonical behaviour.
- The **trailing-digit** fallback added on the `main` branch is an additive
  extension for brand variants whose USB iSerialNumber appends one extra digit
  to the printed 11-character serial (for example a Casio CAY01 label reads
  `SCAEE035901` while the descriptor returns `SCAEE0359010`). It only runs when
  the upstream canonical parse cannot produce exactly eight bytes; it drops the
  trailing decimal digit and parses the remaining standard serial. The canonical
  11-character, 16-hex and empty-SN behaviour is untouched. See `src/lib.rs`
  and the fallback tests for vectors.

## Updating this copy

Re-download `auRokku.py` and `LICENSE` from a newer upstream commit, update the
commit/blob/SHA256 values above, and state in the commit message which
protocol behaviour changed. Do not edit the vendored file: keep it byte-for-byte
identical to upstream so the SHA256 stays verifiable.
