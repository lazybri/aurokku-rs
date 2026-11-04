//! Host-side mocks for the HAL traits.
//!
//! These make the whole firmware app testable in `cargo test` without any
//! hardware: the display records the character grid, the input replays events
//! and the clock can be advanced manually.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::hal::{Clock, Display, Input, InputEvent, DISPLAY_COLS, DISPLAY_ROWS};

/// Records a 21×6 character grid instead of pixels.
#[derive(Debug, Clone)]
pub struct MockDisplay {
    rows: Vec<String>,
    flushes: usize,
}

impl Default for MockDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDisplay {
    pub fn new() -> Self {
        Self {
            rows: vec![empty_row(); DISPLAY_ROWS as usize],
            flushes: 0,
        }
    }

    /// Text currently shown on `row` (trailing blanks preserved, so tests can
    /// assert the exact layout).
    pub fn line(&self, row: u8) -> &str {
        &self.rows[row as usize]
    }

    /// All rows joined with `\n` (handy for snapshot assertions).
    pub fn screen(&self) -> String {
        self.rows.join("\n")
    }

    /// Number of `flush` calls so far.
    pub fn flushes(&self) -> usize {
        self.flushes
    }
}

fn empty_row() -> String {
    " ".repeat(DISPLAY_COLS as usize)
}

impl Display for MockDisplay {
    fn clear(&mut self) {
        self.rows = vec![empty_row(); DISPLAY_ROWS as usize];
    }

    fn text(&mut self, row: u8, col: u8, text: &str) {
        let Some(line) = self.rows.get_mut(row as usize) else {
            return;
        };
        let mut chars: Vec<char> = line.chars().collect();
        for (offset, ch) in text.chars().enumerate() {
            if !ch.is_ascii() {
                continue;
            }
            let position = col as usize + offset;
            if position >= DISPLAY_COLS as usize {
                break;
            }
            chars[position] = ch;
        }
        *line = chars.into_iter().collect();
    }

    fn flush(&mut self) {
        self.flushes += 1;
    }
}

/// Replays a fixed list of input events.
#[derive(Debug, Clone, Default)]
pub struct MockInput {
    events: VecDeque<InputEvent>,
}

impl MockInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: InputEvent) {
        self.events.push_back(event);
    }

    pub fn push_all(&mut self, events: impl IntoIterator<Item = InputEvent>) {
        self.events.extend(events);
    }

    pub fn pending(&self) -> usize {
        self.events.len()
    }
}

impl Input for MockInput {
    fn poll(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }
}

/// Manually advanced millisecond clock.
#[derive(Debug, Clone, Default)]
pub struct MockClock {
    now_ms: u64,
}

impl MockClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&mut self, ms: u64) {
        self.now_ms = self.now_ms.saturating_add(ms);
    }

    pub fn set(&mut self, ms: u64) {
        self.now_ms = ms;
    }
}

impl Clock for MockClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

/// Convenience re-export: the scripted phone used by firmware tests.
pub use auroku_core::mock::MockTransport;

/// Build a human-readable error line for assertions/logs.
pub fn describe_error(error: &auroku_core::AuRokkuError) -> String {
    error.to_string()
}
