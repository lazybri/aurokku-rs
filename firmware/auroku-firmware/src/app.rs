//! Firmware event loop: wires input -> UI -> protocol -> display.
//!
//! The loop is deliberately tiny and platform-free: one `tick()` drains pending
//! input events, lets the UI translate them into actions, runs the requested
//! protocol task through `auroku-core`, feeds the typed outcome back to the UI
//! and paints the screen. The platform layer (RP2354 `main`) only has to call
//! `tick()` periodically and forward attach/detach events.
//!
//! MVP note: the protocol task executes synchronously inside `tick()` (the
//! desktop behaviour). docs/plan/06 §6.2 wants it on the second core / in an
//! async task with stage progress; the [`UiAction`]/[`crate::protocol`]
//! boundary keeps that move local.

use auroku_core::{Task, UsbTransport};

use crate::hal::{Clock, Display, Input};
use crate::protocol::{self, DeviceInfo, ProtoConfig};
use crate::ui::{UiAction, UiState};

/// Static firmware configuration (USB IDs and `--type2` behaviour).
#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    /// USB vendor id of the phone (exit handling differs for `0x0FCE`).
    pub vid: u16,
    /// Whether base authentication is required (`--type2` on the desktop CLI).
    pub type2: bool,
}

/// The mockable firmware application.
pub struct App<D: Display, I: Input, C: Clock, T: UsbTransport> {
    display: D,
    input: I,
    clock: C,
    config: AppConfig,
    ui: UiState,
    link: Option<T>,
    device: Option<DeviceInfo>,
}

impl<D: Display, I: Input, C: Clock, T: UsbTransport> App<D, I, C, T> {
    /// Create the app with its platform peripherals.
    pub fn new(display: D, input: I, clock: C, config: AppConfig) -> Self {
        Self {
            display,
            input,
            clock,
            config,
            ui: UiState::new(),
            link: None,
            device: None,
        }
    }

    /// Platform callback: a phone was enumerated/configured and a transport is
    /// ready. `device.sn` should be the iSerialNumber string when available.
    pub fn attach(&mut self, transport: T, device: DeviceInfo) {
        self.device = Some(device.clone());
        self.link = Some(transport);
        self.ui.on_attach(device);
    }

    /// Platform callback: the phone was unplugged / the bus was reset.
    pub fn detach(&mut self) {
        self.link = None;
        self.device = None;
        self.ui.on_detach();
    }

    /// Whether a phone transport is currently attached.
    pub fn is_attached(&self) -> bool {
        self.link.is_some()
    }

    /// Run one UI iteration. Call every few milliseconds from the platform loop.
    pub fn tick(&mut self) {
        while let Some(event) = self.input.poll() {
            let now_ms = self.clock.now_ms();
            match self.ui.handle(event, now_ms) {
                UiAction::None => {}
                UiAction::Submit(task) => self.submit(task),
            }
        }

        self.ui.render(&mut self.display);
        self.display.flush();
    }

    fn submit(&mut self, task: Task) {
        let (link, device) = match (self.link.as_mut(), self.device.as_ref()) {
            (Some(link), Some(device)) => (link, device.clone()),
            _ => {
                self.ui.on_no_device();
                return;
            }
        };

        let config = ProtoConfig {
            vid: self.config.vid,
            type2: self.config.type2,
            sn: device.sn.clone(),
        };
        let outcome = protocol::run_task(link, &config, &task);
        self.ui.on_outcome(&outcome);
    }

    /// UI state (tests inspect the current page through this).
    pub fn ui(&self) -> &UiState {
        &self.ui
    }

    /// Display backend (tests read the recorded character grid).
    pub fn display(&self) -> &D {
        &self.display
    }

    /// Input backend (tests queue additional events).
    pub fn input_mut(&mut self) -> &mut I {
        &mut self.input
    }

    /// Clock backend (tests advance long-press timers).
    pub fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
    }

    /// Active configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Test/diagnostic access to the attached transport (e.g. to assert that a
    /// scripted phone saw exactly the expected frames).
    pub fn link_mut(&mut self) -> Option<&mut T> {
        self.link.as_mut()
    }
}
