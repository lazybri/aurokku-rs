#![no_std]
#![forbid(unsafe_code)]

//! auRokku 协议核心的 no_std 可行性 spike。
//!
//! 本 crate 把 main 分支 src/lib.rs 中与 USB 主机栈、clap、serde_json、std 时间
//! 无关的“纯协议层”原样抽出，用来回答一个问题：
//!
//! > lib.rs 的编解码 / Type2Auth / 帧收发逻辑，能否在 no_std + alloc 的 MCU
//! > 目标（RP2354 / Cortex-M33）上编译并通过同一批单测？
//!
//! 迁移纪律：
//! - 算法逐行照抄 main 的实现，不顺手“优化”，便于用同一组测试向量对照；
//! - 每个函数上方注释标注 main src/lib.rs 的原始行号，方便 review 与后续上游同步；
//! - 只允许两处语义变化：① std::error::Error -> core::error::Error（Rust 1.81+ 稳定）；
//!   ② serde_json::Value 返回值 -> 强类型 struct（JSON 只属于 CLI/GUI 适配层）；
//! - 时间/超时不再由核心层用 std::time::Instant 测量，而是交给 UsbTransport 实现方
//!   （MCU 上是 USBH 轮询预算，host 上是 libusb timeout），核心只处理“读到了 / 没读到”。

extern crate alloc;

pub mod mock;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use md5::{Digest, Md5};

// ==============================================================================
// 1. 错误类型（main src/lib.rs:41-116）
// ==============================================================================

/// Errors surfaced by the protocol layer.
///
/// 与 main 的 AuRokkuError 变体一一对应；唯一差别是去掉 impl From<rusb::Error>
/// （rusb 是 host USB 后端，属于 platform 层）并改用 core::error::Error。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuRokkuError {
    /// Mirrors Python's ValueError.
    Value(String),
    /// Mirrors Python's RuntimeError.
    Runtime(String),
    /// Mirrors Python's TimeoutError.
    Timeout(String),
    /// The device cannot be opened until the right driver is bound (host only;
    /// 保留变体是为了让上层 match 分支与 main 一致).
    Driver(String),
}

impl AuRokkuError {
    /// Name of the equivalent Python exception.
    pub fn kind(&self) -> &'static str {
        match self {
            AuRokkuError::Value(_) => "ValueError",
            AuRokkuError::Runtime(_) => "RuntimeError",
            AuRokkuError::Timeout(_) => "TimeoutError",
            AuRokkuError::Driver(_) => "DriverError",
        }
    }
}

impl fmt::Display for AuRokkuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuRokkuError::Value(msg)
            | AuRokkuError::Runtime(msg)
            | AuRokkuError::Timeout(msg)
            | AuRokkuError::Driver(msg) => f.write_str(msg),
        }
    }
}

// main 里是 impl std::error::Error for AuRokkuError（lib.rs:77）。
// core::error::Error 自 Rust 1.81 稳定，无需任何依赖即可在 no_std 下保留该 impl。
impl core::error::Error for AuRokkuError {}

/// Result type used by every public protocol operation.
pub type AuRokkuResult<T> = Result<T, AuRokkuError>;

// ==============================================================================
// 2. Protocol codec / SN parsing（main src/lib.rs:123-216）
// ==============================================================================

/// Split each byte into two nibble bytes: high nibble and low nibble (00..0F).
///
/// main: encode_nibbles (src/lib.rs:123-130)。
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
/// main: decode_nibbles (src/lib.rs:133-145)。
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
        .map(|c| (c[0] << 4) | c[1])
        .collect())
}

/// Decode SERI 0xDC lockcode response bytes into an ASCII string.
///
/// main: decode_lockcode_digits (src/lib.rs:148-162)。String 来自 alloc。
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

/// Parse a serial number into the 8 raw Type2Auth bytes plus the canonical name.
///
/// main: parse_sn_to_type2auth (src/lib.rs:170-192)。hex::decode 在 no_std 下由
/// hex 的 alloc feature 提供，返回 Vec。
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

/// argparse-style hex USB endpoint parser.
///
/// main: parse_usb_endpoint (src/lib.rs:195-207)。只被 CLI 参数解析使用，
/// 在 MCU 固件里通常不需要；保留在此证明它同样无 std 依赖。
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

// ==============================================================================
// 3. Type2Auth calculations（main src/lib.rs:223-296）
// ==============================================================================

/// Reverse the 8 bits of a single byte.
///
/// main: bitrev8 (src/lib.rs:223-225)。
pub fn bitrev8(b: u8) -> u8 {
    b.reverse_bits()
}

/// Derive key material K from the 8 raw Type2Auth bytes.
///
/// main: derive_type2auth_key (src/lib.rs:228-246)。
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

/// main: md5_digest (src/lib.rs:248-257)。md-5 关闭 std 后只依赖 digest 的
/// no_std 路径，不引入 std / getrandom / 平台代码。
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

/// Compute the SERI 0x53 authentication response packet.
///
/// main: compute_type2auth_request (src/lib.rs:260-296)。
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

// ==============================================================================
// 4. Helpers（main src/lib.rs:302-314）
// ==============================================================================

/// main: hex_spaced (src/lib.rs:302-307)。
pub fn hex_spaced(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// main: find_subslice (src/lib.rs:309-314)。
pub fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ==============================================================================
// 5. 传输抽象（main src/lib.rs:678-758 的 recv/read_until + 603-611 的日志）
// ==============================================================================

/// 一次 USB 端点事务的最小抽象。
///
/// 这是本 spike 对 main AuRokkuDevice 做的最关键的解耦：main 的 send/recv
/// 直接调用 rusb DeviceHandle，并且用 std::time::Instant 做超时；核心层只要求
/// “写一整帧 / 读一段字节”，超时预算由实现方决定（host: libusb timeout；
/// MCU: USBH 轮询 + 调用方给的 deadline）。
///
/// 契约：
/// - write 必须写完 data 全部字节，否则返回错误（对应 main 的短写检查）；
/// - read 阻塞到至少 1 字节可用，返回本次读到的字节数（可小于 buf.len()）；
///   超时/无数据时返回 AuRokkuError::Timeout。返回 Ok(0) 视为超时（核心会转成
///   Timeout 错误），这样实现方不必构造错误类型。
pub trait UsbTransport {
    fn write(&mut self, data: &[u8]) -> AuRokkuResult<()>;
    fn read(&mut self, buf: &mut [u8]) -> AuRokkuResult<usize>;
}

/// 单次端点读取的暂存区大小。main 用固定的 [0u8; 512]（src/lib.rs:680）；
/// MCU 上 64 字节足够（SERI 最长帧 256，端点包通常 64），可按需调整。
pub const ENDPOINT_CHUNK: usize = 64;

/// 帧收发器：复刻 main recv 的缓冲/握手过滤语义，但不做时间测量。
pub struct FrameIo<T: UsbTransport> {
    transport: T,
    rx: Vec<u8>,
    awaiting_handshake: bool,
    /// main 把 0xAF 预回声映射成 JSON 事件（src/lib.rs:695-703）；
    /// 核心层只记录原始 opcode，JSON 由上层适配。
    async_events: Vec<u8>,
}

impl<T: UsbTransport> FrameIo<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            rx: Vec::new(),
            awaiting_handshake: false,
            async_events: Vec::new(),
        }
    }

    pub fn transport(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn into_inner(self) -> T {
        self.transport
    }

    /// 当前已缓冲但尚未被消费的字节（调试用）。
    pub fn buffered(&self) -> &[u8] {
        &self.rx
    }

    /// 取下已观察到的异步事件 opcode（main: async_events）。
    pub fn take_async_events(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.async_events)
    }

    /// 进入/退出握手的 0xAF 预回声过滤窗口（main: awaiting_handshake）。
    pub fn begin_handshake(&mut self) {
        self.awaiting_handshake = true;
    }

    pub fn end_handshake(&mut self) {
        self.awaiting_handshake = false;
    }

    pub fn send(&mut self, data: &[u8]) -> AuRokkuResult<()> {
        self.transport.write(data)
    }

    /// 读取恰好 expected_len 字节。
    ///
    /// 对应 main recv（src/lib.rs:678-713）：循环读端点、累积到 rx_buffer、
    /// 握手窗口内剥掉前导 0xAF，最后 drain 出 expected_len 字节。
    pub fn recv(&mut self, expected_len: usize) -> AuRokkuResult<Vec<u8>> {
        let mut scratch = [0u8; ENDPOINT_CHUNK];
        while self.rx.len() < expected_len {
            let n = self.transport.read(&mut scratch)?;
            if n == 0 {
                return Err(AuRokkuError::Timeout(format!(
                    "Timeout waiting for {expected_len} bytes; buffered {}",
                    hex_spaced(&self.rx)
                )));
            }
            self.rx.extend_from_slice(&scratch[..n]);
            if self.awaiting_handshake {
                while self.rx.first() == Some(&0xAF) {
                    self.async_events.push(0xAF);
                    self.rx.remove(0);
                }
            }
        }
        Ok(self.rx.drain(..expected_len).collect())
    }

    /// 读到终止字节为止，上限 limit 字节。
    ///
    /// 对应 main read_until（src/lib.rs:734-758）；read_until 用于 0xDC 0x00
    /// lockcode 读取（以 0xAA 结尾）。
    pub fn read_until(&mut self, terminator: u8, limit: usize) -> AuRokkuResult<Vec<u8>> {
        let mut data = Vec::new();
        while data.len() < limit {
            data.extend_from_slice(&self.recv(1)?);
            if data.last() == Some(&terminator) {
                return Ok(data);
            }
        }
        Err(AuRokkuError::Runtime(
            "Response exceeds bounded limit".to_string(),
        ))
    }

    /// 丢弃本地缓冲（main drain 的纯软件部分；硬件排空由 transport 负责）。
    pub fn drain(&mut self) {
        self.rx.clear();
    }
}

// ==============================================================================
// 6. UIMLK 操作（main src/lib.rs:1318-1354、1249-1263、1281-1292）
// ==============================================================================

pub const CMD_BASE_HANDSHAKE: [u8; 2] = [0x1C, 0x05];
pub const CMD_UIMLK_SWITCH: u8 = 0xE7;
pub const CMD_READ_STATE: u8 = 0x54;
pub const CMD_EXIT_UIMLK: u8 = 0x56;
pub const STATE_FRAME_LEN: usize = 10;

/// main read_state 返回的 serde_json::Value 的强类型版本（src/lib.rs:1319-1338）。
///
/// 字段与 JSON key 一一对应：control / lock_value / binding_cleared / raw_hex。
/// MCU 固件不需要 serde_json，因此核心层只暴露 struct；CLI/GUI 在适配层做
/// impl From<UimLockState> for serde_json::Value 即可保持输出兼容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UimLockState {
    pub control: u8,
    pub lock_value_hex: String,
    pub binding_cleared: bool,
    pub raw: Vec<u8>,
}

/// 解析 0x74 UIM lock state 帧（main：read_state 中的校验 + decode，src/lib.rs:1321-1337）。
///
/// 帧布局（10 字节）：74 | control(0/1) | 8 个 nibble 字节。
pub fn parse_uimlk_state_frame(resp: &[u8]) -> AuRokkuResult<UimLockState> {
    if resp.len() != STATE_FRAME_LEN || resp[0] != 0x74 || (resp[1] != 0 && resp[1] != 1) {
        return Err(AuRokkuError::Runtime(format!(
            "Invalid state reply: {}",
            hex_spaced(resp)
        )));
    }

    let value = hex::encode(decode_nibbles(&resp[2..])?);
    let binding_cleared = value == "ffffffff";
    Ok(UimLockState {
        control: resp[1],
        lock_value_hex: value,
        binding_cleared,
        raw: resp.to_vec(),
    })
}

/// 建立 base 会话（main enter_base 的简化版，src/lib.rs:1219-1246）。
///
/// 与 main 的差异：略去 drain(50ms)、非 0x0FCE 设备的 0xD8 预退出、--type2
/// 的 base 认证（这些分别属于时序/设备策略/认证子流程，见报告第 5 节）。
/// 保留 1C 05 握手回显校验与 0xAF 预回声过滤。
pub fn enter_base<T: UsbTransport>(io: &mut FrameIo<T>) -> AuRokkuResult<()> {
    io.begin_handshake();
    let result = (|| {
        io.send(&CMD_BASE_HANDSHAKE)?;
        let echo = io.recv(2)?;
        if echo != CMD_BASE_HANDSHAKE {
            return Err(AuRokkuError::Runtime(format!(
                "Base handshake echo mismatch: {}",
                hex_spaced(&echo)
            )));
        }
        Ok(())
    })();
    io.end_handshake();
    result
}

/// 切换到 UIMLK 服务（main enter_uimlk 去掉 Type2Auth 认证的版本，src/lib.rs:1249-1263）。
pub fn enter_uimlk<T: UsbTransport>(io: &mut FrameIo<T>) -> AuRokkuResult<()> {
    enter_base(io)?;

    io.send(&[CMD_UIMLK_SWITCH])?;
    let switch_echo = io.recv(1)?;
    if switch_echo != [CMD_UIMLK_SWITCH] {
        return Err(AuRokkuError::Runtime(format!(
            "Service switch to UIMLK failed: {}",
            hex_spaced(&switch_echo)
        )));
    }
    Ok(())
}

/// 读取 UIM lock 状态（main read_state 的传输部分，src/lib.rs:1319-1321）。
pub fn read_uimlk_state<T: UsbTransport>(io: &mut FrameIo<T>) -> AuRokkuResult<UimLockState> {
    io.send(&[CMD_READ_STATE])?;
    let resp = io.recv(STATE_FRAME_LEN)?;
    parse_uimlk_state_frame(&resp)
}

/// 退出 UIMLK（main exit_uimlk，src/lib.rs:1281-1292）。
pub fn exit_uimlk<T: UsbTransport>(io: &mut FrameIo<T>) -> AuRokkuResult<()> {
    io.send(&[CMD_EXIT_UIMLK])?;
    let resp = io.recv(1)?;
    if resp != [CMD_EXIT_UIMLK] {
        return Err(AuRokkuError::Runtime(format!(
            "Invalid exit reply: {}",
            hex_spaced(&resp)
        )));
    }
    Ok(())
}

/// main read_uimlk（src/lib.rs:1341-1354）的一次完整会话结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UimLockReadOutcome {
    pub state: UimLockState,
    /// main 的 finish_uimlk 把 0x56 退出失败的 acknowledged 记为 false 而不上抛错误
    /// （src/lib.rs:1295-1312）；这里用布尔位保留同一语义。
    pub exit_acknowledged: bool,
}

/// 读 UIM lock 的完整操作状态机：握手 -> 切服务 -> 读状态 -> 退出。
///
/// 对应 main read_uimlk + enter_uimlk + read_state + finish_uimlk 的组合
/// （src/lib.rs:1341-1354 / 1249-1263 / 1319-1338 / 1295-1312）。
/// 整个函数只依赖 UsbTransport，不含任何 host / std / 时间 API。
pub fn read_uimlk<T: UsbTransport>(io: &mut FrameIo<T>) -> AuRokkuResult<UimLockReadOutcome> {
    enter_uimlk(io)?;
    let state = read_uimlk_state(io);
    let exit_acknowledged = exit_uimlk(io).is_ok();
    Ok(UimLockReadOutcome {
        state: state?,
        exit_acknowledged,
    })
}

// ==============================================================================
// 7. Tests —— 前 13 个逐字迁移自 main src/lib.rs:2906-3139 的相关单测
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

    // -- 以下为 spike 新增：帧解析单测（不是从 main 迁移）---------------------

    #[test]
    fn state_frame_parses_bound_value() {
        let frame = [0x74, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let state = parse_uimlk_state_frame(&frame).unwrap();
        assert_eq!(state.control, 0);
        assert_eq!(state.lock_value_hex, "12345678");
        assert!(!state.binding_cleared);
        assert!(&state.raw[..] == &frame[..]);
    }

    #[test]
    fn state_frame_parses_cleared_binding() {
        let mut frame = [0u8; 10];
        frame[0] = 0x74;
        frame[1] = 0x01;
        frame[2..].copy_from_slice(&[0x0F; 8]);
        let state = parse_uimlk_state_frame(&frame).unwrap();
        assert_eq!(state.control, 1);
        assert_eq!(state.lock_value_hex, "ffffffff");
        assert!(state.binding_cleared);
    }

    #[test]
    fn state_frame_rejects_bad_control_and_length() {
        assert!(parse_uimlk_state_frame(&[0x74, 0x02, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(parse_uimlk_state_frame(&[0x74, 0x00]).is_err());
        assert!(parse_uimlk_state_frame(&[0x75, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
