//! In-memory [`UsbTransport`] for host tests and firmware mock builds.
//!
//! The mock is *command/response* driven rather than a bare byte stream: each
//! scripted step names the exact frame the protocol core is expected to write
//! and the bytes the (simulated) phone answers with. This mirrors the real
//! protocol far better than a flat stream and lets tests assert the command
//! order (e.g. `1C 05 -> E7 -> 52 -> 53 ... -> 54 -> 56`).
//!
//! Reads can be fragmented with [`MockTransport::with_chunk`] to exercise the
//! buffering logic, and an exhausted script produces timeouts whose clock
//! advances realistically so `recv`/`read_until` deadlines terminate.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::{AuRokkuError, AuRokkuResult};
use crate::transport::UsbTransport;

/// One scripted request/response pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockStep {
    /// Frame the core must write for this step.
    pub expect_write: Vec<u8>,
    /// Bytes the simulated device returns for that frame (may be empty).
    pub response: Vec<u8>,
}

/// Scripted, deterministic transport.
#[derive(Debug, Clone)]
pub struct MockTransport {
    /// Frames written by the core, in order.
    pub writes: Vec<Vec<u8>>,
    /// Scripted steps that have not been consumed yet.
    steps: VecDeque<MockStep>,
    /// Bytes of the current step's response not yet delivered.
    rx: VecDeque<u8>,
    /// Max bytes returned by one `read` call (fragmentation simulation).
    chunk: usize,
    /// Progressed by writes, delays and timeout reads.
    now_ms: u64,
    /// Protocol violations seen so far (wrong frame / unexpected write).
    pub violations: Vec<String>,
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            writes: Vec::new(),
            steps: VecDeque::new(),
            rx: VecDeque::new(),
            chunk: usize::MAX,
            now_ms: 0,
            violations: Vec::new(),
        }
    }

    /// Limit the number of bytes returned by a single `read` (endpoint chunking).
    pub fn with_chunk(mut self, chunk: usize) -> Self {
        self.chunk = chunk.max(1);
        self
    }

    /// Append one scripted request/response step.
    pub fn on_write(mut self, expect_write: &[u8], response: &[u8]) -> Self {
        self.push_step(expect_write, response);
        self
    }

    /// Append one scripted request/response step in place.
    pub fn push_step(&mut self, expect_write: &[u8], response: &[u8]) -> &mut Self {
        self.steps.push_back(MockStep {
            expect_write: expect_write.to_vec(),
            response: response.to_vec(),
        });
        self
    }

    /// Number of scripted steps left (0 means the script was fully consumed).
    pub fn remaining_steps(&self) -> usize {
        self.steps.len()
    }

    /// Bytes of the current step's response still buffered.
    pub fn buffered(&self) -> usize {
        self.rx.len()
    }

    /// Whether any protocol violation was recorded.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Panic with a readable message if the script was not followed exactly.
    ///
    /// Tests should call this after an operation so protocol-order regressions
    /// fail loudly.
    pub fn assert_clean(&self) {
        assert!(
            self.violations.is_empty(),
            "mock transport protocol violations: {:?}",
            self.violations
        );
        assert!(
            self.steps.is_empty(),
            "mock transport has {} unconsumed scripted steps",
            self.steps.len()
        );
        assert!(
            self.rx.is_empty(),
            "mock transport has {} unconsumed response bytes",
            self.rx.len()
        );
    }
}

impl UsbTransport for MockTransport {
    fn write(&mut self, data: &[u8], timeout: Duration) -> AuRokkuResult<()> {
        self.writes.push(data.to_vec());
        self.now_ms = self.now_ms.saturating_add(1);

        match self.steps.pop_front() {
            Some(step) => {
                if step.expect_write != data {
                    self.violations.push(format!(
                        "write mismatch: expected {:02x?}, got {:02x?}",
                        step.expect_write, data
                    ));
                }
                self.rx.extend(step.response);
                let _ = timeout;
                Ok(())
            }
            None => {
                self.violations.push(format!(
                    "unexpected write with no scripted step left: {data:02x?}"
                ));
                Err(AuRokkuError::Runtime(
                    "mock transport: unexpected write".to_string(),
                ))
            }
        }
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> AuRokkuResult<usize> {
        if self.rx.is_empty() {
            // No data: behave like a real transfer timing out, and advance the
            // clock by the requested timeout so deadline loops terminate.
            let advance = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
            self.now_ms = self.now_ms.saturating_add(advance.max(1));
            return Err(AuRokkuError::Timeout(
                "mock transport: no scripted response".to_string(),
            ));
        }

        let n = buf.len().min(self.chunk).min(self.rx.len());
        for slot in buf.iter_mut().take(n) {
            if let Some(byte) = self.rx.pop_front() {
                *slot = byte;
            }
        }
        self.now_ms = self.now_ms.saturating_add(1);
        Ok(n)
    }

    fn delay(&mut self, duration: Duration) {
        self.now_ms = self
            .now_ms
            .saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX));
    }

    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}
