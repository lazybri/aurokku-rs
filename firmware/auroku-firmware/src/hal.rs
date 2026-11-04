//! Hardware abstraction traits.
//!
//! The firmware logic never touches a HAL crate directly; the RP2354 platform
//! layer implements these traits over `rp235x-hal` + `ssd1306` + EC11 GPIOs
//! (docs/plan/06 §4.2), and host tests implement them with the mocks in
//! [`crate::mock`].
//!
//! Text-grid display: the MVP UI is ASCII text on a 128×64 OLED, 21×6 characters
//! with a 6×10 font (docs/plan/09 §1.1). Rendering to pixels is the platform
//! driver's job (`ssd1306` + `embedded-graphics`); this crate only decides what
//! text goes where.

/// Display columns at the MVP 6×10 font (128 px / 6).
pub const DISPLAY_COLS: u8 = 21;

/// Display rows at the MVP 6×10 font (64 px / 10).
pub const DISPLAY_ROWS: u8 = 6;

/// A 128×64 monochrome text display.
pub trait Display {
    /// Clear the frame buffer.
    fn clear(&mut self);

    /// Draw `text` at `(row, col)` in the 21×6 character grid.
    ///
    /// Implementations must clip instead of panicking: UI code may write a full
    /// line at column 0 with 21 characters, and a few pages intentionally
    /// overflow on long error strings.
    fn text(&mut self, row: u8, col: u8, text: &str);

    /// Push the frame buffer to the panel (blocking I2C flush is acceptable on
    /// the UI task, docs/plan/06 §4.2).
    fn flush(&mut self);
}

/// Input events produced by the EC11 encoder + BACK button.
///
/// The platform driver owns debouncing (20 ms, docs/plan/09 §4.3) and long-press
/// detection: it emits [`InputEvent::EncoderLongPress`] once the 1.5 s threshold
/// is crossed (docs/plan/09 §3.2). The UI only decides what each event means.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// Encoder rotated clockwise (next item).
    EncoderCw,
    /// Encoder rotated counter-clockwise (previous item).
    EncoderCcw,
    /// Encoder pushed (short press = OK).
    EncoderPress,
    /// Encoder held for >= 1.5 s (destructive confirmation, docs/plan/09 §3.2).
    EncoderLongPress,
    /// BACK button pressed (up one level / cancel).
    BackPress,
}

/// Monotonic millisecond clock (used for long-press confirmation timing).
pub trait Clock {
    fn now_ms(&self) -> u64;
}

/// Non-blocking input source.
pub trait Input {
    /// Return the next pending event, or `None` when the queue is empty.
    fn poll(&mut self) -> Option<InputEvent>;
}
