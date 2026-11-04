#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

//! RP2354 firmware logic for auRokku.
//!
//! This crate is the **hardware-independent** half of the handheld firmware that
//! will run on an RP2354A board (docs/plan/06, 07, 09). It is a `no_std` + `alloc`
//! library so it can be compiled for `thumbv8m.main-none-eabihf`, while all of
//! its logic is driven from host-side tests through mocks:
//!
//! ```text
//! ┌──────────────────────────── app.rs (event loop) ───────────────────────────┐
//! │  tick(): input.poll() -> ui.handle() -> protocol::run_task() -> ui.render() │
//! └───────────────┬───────────────────┬────────────────────┬──────────────────┘
//!                 │                   │                    │
//!           hal.rs (traits)      ui.rs (menus)      protocol.rs (core wrapper)
//!                 │                   │                    │
//!    ssd1306 / EC11 / rp235x-hal   menu tree + pages   auroku-core Session
//!                 │
//!          mock.rs (host tests): MockDisplay / MockInput / MockClock
//! ```
//!
//! The USB host stack (cotton-usb-host or TinyUSB) is **not** part of this crate
//! yet: the platform layer supplies any `UsbTransport` implementation, and
//! `auroku_core::mock::MockTransport` supplies a scripted phone on the host.
//! That is what makes the firmware codebase mockable before the custom PCB
//! exists (docs/plan/06 §7.2 G1 gate).
//!
//! Scope of this crate (MVP, docs/plan/09):
//! - OLED text UI: main menu, result pages, error pages, confirm/long-press;
//! - protocol execution for the 12 `Task`s through `auroku-core`;
//! - attach/detach (hot-plug) state and SN display;
//! - all logic host-testable with mocks (`cargo test`).

extern crate alloc;

pub mod app;
pub mod hal;
pub mod mock;
pub mod protocol;
pub mod ui;

pub use app::{App, AppConfig};
pub use hal::{Clock, Display, Input, InputEvent, DISPLAY_COLS, DISPLAY_ROWS};
pub use protocol::{DeviceInfo, ProtoConfig, ProtoOutcome};
pub use ui::UiState;
