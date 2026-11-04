//! Transport abstraction: the only thing the protocol core needs from a USB
//! host stack.
//!
//! Synchronous on purpose (docs/plan/08 §6.3b): the desktop backend (`rusb`) is
//! blocking anyway, and a C/TinyUSB firmware can call the core from its poll
//! loop without an executor. The pure-Rust (cotton/RTIC) firmware can add an
//! async adapter on top later; operation sequences stay identical.
//!
//! Contract:
//! - [`UsbTransport::write`] must write the whole frame or return an error
//!   (the core never issues partial writes; the longest frame is 17 bytes);
//! - [`UsbTransport::read`] blocks up to `timeout` and returns the number of
//!   bytes read. `Ok(0)` is treated as a timeout; `Err(Timeout)` is also a
//!   timeout. Any other error aborts the current operation;
//! - [`UsbTransport::delay`] implements the protocol-level 50 ms silence after
//!   `0xD8` (it is *not* a retry backoff);
//! - [`UsbTransport::now_ms`] is a monotonic millisecond clock used for the
//!   per-transfer deadlines that the original code implements with
//!   `std::time::Instant`.

use core::time::Duration;

use crate::error::AuRokkuResult;

/// Default control transfer timeout used by the protocol layer.
///
/// Original: `DEFAULT_TIMEOUT` (`src/lib.rs:27`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Timeout for a whole OUT write (2 s in the original `send`, `src/lib.rs:663`).
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_millis(2000);

/// Per-read slice timeout used while accumulating a frame (`src/lib.rs:690`).
pub const READ_SLICE_TIMEOUT: Duration = Duration::from_millis(500);

/// `drain(50)` timeout used before entering base / before probing.
pub const DRAIN_TIMEOUT: Duration = Duration::from_millis(50);

/// Scratch buffer size for one endpoint read (the original uses 512 B).
pub const ENDPOINT_CHUNK: usize = 512;

/// Upper bound of reads issued by one `drain` call (`src/lib.rs:723`).
pub const MAX_DRAIN_READS: usize = 20;

/// A minimal, synchronous USB endpoint transport.
///
/// Implementations provide exactly the four primitives the protocol needs;
/// descriptors, interface claiming, endpoint discovery and hot-plug handling
/// stay on the platform side.
pub trait UsbTransport {
    /// Write the whole `data` frame, or fail.
    fn write(&mut self, data: &[u8], timeout: Duration) -> AuRokkuResult<()>;

    /// Read at least one byte into `buf`. `Ok(0)` or `Err(Timeout)` means the
    /// timeout expired without data; any other error aborts the operation.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> AuRokkuResult<usize>;

    /// Protocol-level silence (only used after `0xD8`).
    fn delay(&mut self, duration: Duration);

    /// Monotonic millisecond clock.
    fn now_ms(&self) -> u64;
}

/// Blanket forwarding impl so a `&mut T` transport can be passed to
/// [`crate::Session::new`] while the caller keeps ownership of `T`
/// (the firmware app reuses one transport across attach/detach cycles).
impl<T: UsbTransport + ?Sized> UsbTransport for &mut T {
    fn write(&mut self, data: &[u8], timeout: Duration) -> AuRokkuResult<()> {
        (**self).write(data, timeout)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> AuRokkuResult<usize> {
        (**self).read(buf, timeout)
    }

    fn delay(&mut self, duration: Duration) {
        (**self).delay(duration)
    }

    fn now_ms(&self) -> u64 {
        (**self).now_ms()
    }
}
