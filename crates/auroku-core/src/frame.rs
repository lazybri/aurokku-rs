//! Frame I/O: buffering, deadline handling and the `0xAF` pre-echo filter.
//!
//! Reproduces the semantics of the original `recv` / `read_until` / `drain` /
//! `read_probe_response` helpers (`src/lib.rs:678-758`, `1048-1076`) on top of
//! the [`UsbTransport`] abstraction. Timeouts are measured with the transport's
//! monotonic clock instead of `std::time::Instant`.

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::time::Duration;

use crate::codec::hex_spaced;
use crate::error::{AuRokkuError, AuRokkuResult};
use crate::transport::{UsbTransport, ENDPOINT_CHUNK, MAX_DRAIN_READS, READ_SLICE_TIMEOUT};

/// Async notifications observed while reading (originally `async_events` JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncEvent {
    /// `0xAF` delimiter seen before the handshake echo (`src/lib.rs:695-703`).
    PreEchoDelimiter,
    /// Extra `0xE7` acknowledgement before a UIMLK challenge (`src/lib.rs:1186-1189`).
    ExtraUimlkAck,
}

impl AsyncEvent {
    /// Opcode string used in the original JSON (`"af"` / `"e7"`).
    pub fn opcode(self) -> &'static str {
        match self {
            AsyncEvent::PreEchoDelimiter => "af",
            AsyncEvent::ExtraUimlkAck => "e7",
        }
    }

    /// Meaning string used in the original JSON.
    pub fn meaning(self) -> &'static str {
        match self {
            AsyncEvent::PreEchoDelimiter => "pre_echo_delimiter",
            AsyncEvent::ExtraUimlkAck => "extra_uimlk_ack_before_challenge",
        }
    }
}

/// Convert a `Duration` to whole milliseconds, saturating (protocol timeouts are
/// tiny; the saturating cast only exists to keep `no_std` arithmetic total).
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Buffered frame reader/writer bound to one [`UsbTransport`].
pub struct FrameIo<T: UsbTransport> {
    transport: T,
    rx: Vec<u8>,
    awaiting_handshake: bool,
    async_events: Vec<AsyncEvent>,
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

    /// Borrow the underlying transport (platform code may need attach/detach).
    pub fn transport(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Consume the frame I/O and return the transport.
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Bytes buffered but not yet consumed (debug/assertions).
    pub fn buffered(&self) -> &[u8] {
        &self.rx
    }

    /// Take the async events observed so far.
    pub fn take_async_events(&mut self) -> Vec<AsyncEvent> {
        core::mem::take(&mut self.async_events)
    }

    /// Record an async event (used by the session for extra `0xE7` acks).
    pub fn push_async_event(&mut self, event: AsyncEvent) {
        self.async_events.push(event);
    }

    /// Open the `0xAF` pre-echo filtering window (original: `awaiting_handshake`).
    pub fn begin_handshake(&mut self) {
        self.awaiting_handshake = true;
    }

    /// Close the pre-echo filtering window.
    pub fn end_handshake(&mut self) {
        self.awaiting_handshake = false;
    }

    /// Whether the pre-echo filtering window is open.
    pub fn in_handshake(&self) -> bool {
        self.awaiting_handshake
    }

    /// Write one protocol frame.
    ///
    /// Original: `send` (`src/lib.rs:655-676`); the whole-frame check is done by
    /// the transport implementation.
    pub fn send(&mut self, data: &[u8]) -> AuRokkuResult<()> {
        self.transport
            .write(data, crate::transport::DEFAULT_WRITE_TIMEOUT)
    }

    /// Read exactly `expected_len` buffered bytes.
    ///
    /// Original: `recv` (`src/lib.rs:678-713`). Loops until the deadline, reading
    /// 500 ms slices; timeouts are retried, other transport errors propagate.
    pub fn recv(&mut self, expected_len: usize, timeout: Duration) -> AuRokkuResult<Vec<u8>> {
        let deadline = self.transport.now_ms().saturating_add(millis(timeout));
        let mut scratch = [0u8; ENDPOINT_CHUNK];

        while self.rx.len() < expected_len {
            if self.transport.now_ms() >= deadline {
                return Err(AuRokkuError::Timeout(format!(
                    "Timeout waiting for {expected_len} bytes; buffered {}",
                    hex_spaced(&self.rx)
                )));
            }

            match self.transport.read(&mut scratch, READ_SLICE_TIMEOUT) {
                Ok(0) | Err(AuRokkuError::Timeout(_)) => {}
                Ok(n) => {
                    self.rx.extend_from_slice(&scratch[..n]);
                    if self.awaiting_handshake {
                        while self.rx.first() == Some(&0xAF) {
                            self.async_events.push(AsyncEvent::PreEchoDelimiter);
                            self.rx.remove(0);
                        }
                    }
                }
                Err(err) => return Err(err),
            }
        }

        Ok(self.rx.drain(..expected_len).collect())
    }

    /// Read until `terminator` is seen, at most `limit` bytes.
    ///
    /// Original: `read_until` (`src/lib.rs:734-758`).
    pub fn read_until(
        &mut self,
        terminator: u8,
        limit: usize,
        timeout: Duration,
    ) -> AuRokkuResult<Vec<u8>> {
        let deadline = self.transport.now_ms().saturating_add(millis(timeout));
        let mut data = Vec::new();

        while data.len() < limit {
            let remaining = deadline.saturating_sub(self.transport.now_ms());
            if remaining == 0 {
                return Err(AuRokkuError::Timeout(
                    "Missing response terminator".to_string(),
                ));
            }
            data.extend_from_slice(&self.recv(1, Duration::from_millis(remaining))?);
            if data.last() == Some(&terminator) {
                return Ok(data);
            }
        }
        Err(AuRokkuError::Runtime(
            "Response exceeds bounded limit".to_string(),
        ))
    }

    /// Discard stale IN bytes and clear the local buffer.
    ///
    /// Original: `drain` (`src/lib.rs:716-732`); up to 20 reads of `timeout`.
    /// Unlike the original, transport errors other than timeouts are simply
    /// treated as "no more data" - this helper has no return value there either.
    pub fn drain(&mut self, timeout: Duration) {
        self.rx.clear();
        let mut scratch = [0u8; ENDPOINT_CHUNK];
        for _ in 0..MAX_DRAIN_READS {
            match self.transport.read(&mut scratch, timeout) {
                Ok(n) if n > 0 => {}
                _ => break,
            }
        }
    }

    /// Collect a probe response until `limit`, the deadline, or a silent gap.
    ///
    /// Original: `read_probe_response` (`src/lib.rs:1048-1076`). The per-read
    /// timeout is clamped to 1..=200 ms; once some data has arrived, a timeout
    /// ends the collection.
    pub fn read_probe_response(
        &mut self,
        timeout: Duration,
        limit: usize,
    ) -> AuRokkuResult<Vec<u8>> {
        let mut data = core::mem::take(&mut self.rx);
        let deadline = self.transport.now_ms().saturating_add(millis(timeout));
        let mut buf = [0u8; ENDPOINT_CHUNK];

        while data.len() < limit && self.transport.now_ms() < deadline {
            let remaining = deadline - self.transport.now_ms();
            let remaining_ms = remaining.clamp(1, 200);
            let want = (limit - data.len()).min(ENDPOINT_CHUNK);

            match self
                .transport
                .read(&mut buf[..want], Duration::from_millis(remaining_ms))
            {
                Ok(0) => {}
                Ok(n) => data.extend_from_slice(&buf[..n]),
                Err(AuRokkuError::Timeout(_)) => {
                    if !data.is_empty() {
                        break;
                    }
                }
                Err(err) => return Err(err),
            }
        }

        Ok(data)
    }
}
