//! OLED menu UI: pure state machine over [`Display`] + [`InputEvent`].
//!
//! The UI follows docs/plan/09: a main menu covering the 12 protocol tasks,
//! result/error pages, and long-press confirmation for destructive operations.
//! It never talks to USB; the [`crate::app`] event loop turns [`UiAction`]s into
//! protocol calls and feeds outcomes back through [`UiState::on_outcome`].
//!
//! This module is the contract between the UI implementation and the app:
//! - [`UiState::handle`] maps one input event (plus the current clock) to an
//!   optional action for the app;
//! - [`UiState::render`] paints the current screen through the HAL;
//! - [`UiState::on_attach`] / [`UiState::on_detach`] / [`UiState::on_outcome`]
//!   carry platform/protocol events into the UI state.
//!
//! The current implementation is a working skeleton (flat menu + result page);
//! the full pages (UIM lock submenu, PIN/ICCID, hex paging, SN entry) are being
//! implemented on top of this contract.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use auroku_core::{Report, Task};

use crate::hal::{Display, InputEvent, DISPLAY_COLS, DISPLAY_ROWS};
use crate::protocol::{DeviceInfo, ProtoOutcome};

/// What the UI asks the app to do after an input event.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    /// Nothing to do (navigation, redraw, ...).
    None,
    /// Run this task against the attached phone.
    Submit(Task),
}

/// Menu entries. Kept as a `Copy` id so static tables are possible; the mapping
/// to [`Task`] happens in [`MenuId::task`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MenuId {
    Status,
    ReadUimlk,
    WriteUimlk,
    ClearUimlk,
    ReadIccid,
    ReadPin,
    ResetPin,
    MaintenanceBit,
    BatchBasic,
    ProbeSeri,
    ExitUimlk,
    Exit,
}

impl MenuId {
    /// Menu order (docs/plan/09 §3.1).
    pub const ALL: [MenuId; 12] = [
        MenuId::Status,
        MenuId::ReadUimlk,
        MenuId::WriteUimlk,
        MenuId::ClearUimlk,
        MenuId::ReadIccid,
        MenuId::ReadPin,
        MenuId::ResetPin,
        MenuId::MaintenanceBit,
        MenuId::BatchBasic,
        MenuId::ProbeSeri,
        MenuId::ExitUimlk,
        MenuId::Exit,
    ];

    /// Short label that fits the 21×6 grid.
    pub fn label(self) -> &'static str {
        match self {
            MenuId::Status => "Status",
            MenuId::ReadUimlk => "Read UIM lock",
            MenuId::WriteUimlk => "Write UIM lock",
            MenuId::ClearUimlk => "Clear UIM lock",
            MenuId::ReadIccid => "Read ICCID",
            MenuId::ReadPin => "Read PIN",
            MenuId::ResetPin => "Reset PIN",
            MenuId::MaintenanceBit => "Maintenance bit",
            MenuId::BatchBasic => "Batch: basic",
            MenuId::ProbeSeri => "Probe SERI",
            MenuId::ExitUimlk => "Exit UIMLK",
            MenuId::Exit => "Exit base",
        }
    }

    /// Destructive operations need a long-press confirmation (docs/plan/09 §3.2).
    pub fn is_destructive(self) -> bool {
        matches!(
            self,
            MenuId::WriteUimlk
                | MenuId::ClearUimlk
                | MenuId::ResetPin
                | MenuId::MaintenanceBit
                | MenuId::ExitUimlk
        )
    }

    /// Build the protocol task. Placeholders use safe defaults; the full UI will
    /// collect values (lock value, control bit, batch profile) before submitting.
    pub fn task(self) -> Task {
        match self {
            MenuId::Status => Task::Status,
            MenuId::ReadUimlk => Task::ReadUimlk,
            MenuId::WriteUimlk => Task::Writelk {
                value: "00000000".to_string(),
                control: 0,
            },
            MenuId::ClearUimlk => Task::Clearlk,
            MenuId::ReadIccid => Task::ReadIccid,
            MenuId::ReadPin => Task::Readpin,
            MenuId::ResetPin => Task::Resetpin,
            MenuId::MaintenanceBit => Task::MaintenanceBit { control: 1 },
            MenuId::BatchBasic => Task::Batch {
                profile: auroku_core::BatchProfile::Basic,
            },
            MenuId::ProbeSeri => Task::Probe {
                kind: auroku_core::ProbeKind::Seri,
                timeout_ms: 1000,
            },
            MenuId::ExitUimlk => Task::Exituimlk,
            MenuId::Exit => Task::Exit,
        }
    }
}

/// Which page is currently shown.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Screen {
    MainMenu,
    Confirm,
    Result,
}

/// The OLED UI state machine.
pub struct UiState {
    screen: Screen,
    selected: usize,
    pending: Option<MenuId>,
    device: Option<DeviceInfo>,
    lines: Vec<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            screen: Screen::MainMenu,
            selected: 0,
            pending: None,
            device: None,
            lines: Vec::new(),
        }
    }

    /// Platform callback: a phone is attached and configured.
    pub fn on_attach(&mut self, device: DeviceInfo) {
        self.device = Some(device);
        self.screen = Screen::MainMenu;
        self.lines.clear();
    }

    /// Platform callback: the phone is gone (unplug, bus reset, ...).
    pub fn on_detach(&mut self) {
        self.device = None;
        self.screen = Screen::MainMenu;
        self.pending = None;
        self.lines.clear();
    }

    /// There is no attached phone, so a submitted task cannot run.
    pub fn on_no_device(&mut self) {
        self.apply_result_message("NO DEVICE", "Attach phone");
    }

    /// Protocol callback: render the typed outcome.
    pub fn on_outcome(&mut self, outcome: &ProtoOutcome) {
        match &outcome.result {
            Ok(task_outcome) => self.apply_report(&task_outcome.report),
            Err(error) => {
                let message = error.to_string();
                let kind = error.kind();
                self.apply_result_message(kind, &message);
            }
        }
    }

    /// Handle one input event. `now_ms` is currently unused by the skeleton (the
    /// long-press detection lives in the input driver) but is part of the
    /// contract for the timed confirmation design (docs/plan/09 §3.2).
    pub fn handle(&mut self, event: InputEvent, _now_ms: u64) -> UiAction {
        match self.screen {
            Screen::MainMenu => match event {
                InputEvent::EncoderCw => {
                    self.selected = (self.selected + 1) % MenuId::ALL.len();
                    UiAction::None
                }
                InputEvent::EncoderCcw => {
                    self.selected = (self.selected + MenuId::ALL.len() - 1) % MenuId::ALL.len();
                    UiAction::None
                }
                InputEvent::EncoderPress => {
                    let id = MenuId::ALL[self.selected];
                    if id.is_destructive() {
                        self.pending = Some(id);
                        self.screen = Screen::Confirm;
                        UiAction::None
                    } else {
                        UiAction::Submit(id.task())
                    }
                }
                _ => UiAction::None,
            },
            Screen::Confirm => match event {
                InputEvent::EncoderLongPress => match self.pending.take() {
                    Some(id) => {
                        self.screen = Screen::Result;
                        self.lines.clear();
                        UiAction::Submit(id.task())
                    }
                    None => UiAction::None,
                },
                InputEvent::BackPress => {
                    self.pending = None;
                    self.screen = Screen::MainMenu;
                    UiAction::None
                }
                _ => UiAction::None,
            },
            Screen::Result => match event {
                InputEvent::BackPress | InputEvent::EncoderPress => {
                    self.screen = Screen::MainMenu;
                    UiAction::None
                }
                _ => UiAction::None,
            },
        }
    }

    /// Paint the current screen.
    pub fn render(&self, display: &mut impl Display) {
        display.clear();
        match self.screen {
            Screen::MainMenu => self.render_menu(display),
            Screen::Confirm => self.render_confirm(display),
            Screen::Result => self.render_result(display),
        }
    }

    fn render_menu(&self, display: &mut impl Display) {
        let header = match &self.device {
            Some(device) => match &device.sn {
                Some(sn) => format!("auRokku  SN {sn}"),
                None => "auRokku  SN ?".to_string(),
            },
            None => "auRokku  NO DEVICE".to_string(),
        };
        display.text(0, 0, clip(&header));
        display.text(1, 0, "---------------------");

        let visible = (DISPLAY_ROWS as usize).saturating_sub(2);
        let start = self.selected.saturating_sub(visible.saturating_sub(1));
        for (offset, id) in MenuId::ALL.iter().skip(start).take(visible).enumerate() {
            let index = start + offset;
            let marker = if index == self.selected { ">" } else { " " };
            let line = format!("{marker}{}", id.label());
            display.text(2 + offset as u8, 0, clip(&line));
        }
    }

    fn render_confirm(&self, display: &mut impl Display) {
        let id = self.pending.unwrap_or(MenuId::Status);
        display.text(0, 0, "CONFIRM - long press");
        display.text(1, 0, "---------------------");
        display.text(2, 0, clip(id.label()));
        display.text(3, 0, "NOT UNDOABLE");
        display.text(4, 0, "Hold encoder 1.5 s");
        display.text(5, 0, "BACK = cancel");
    }

    fn render_result(&self, display: &mut impl Display) {
        for (row, line) in self.lines.iter().take(DISPLAY_ROWS as usize).enumerate() {
            display.text(row as u8, 0, clip(line));
        }
        if self.lines.is_empty() {
            display.text(0, 0, "Working...");
        }
    }

    /// Fill the result page from a typed report (skeleton: plain text lines).
    fn apply_report(&mut self, report: &Report) {
        self.screen = Screen::Result;
        self.lines.clear();
        let mut lines = Vec::new();
        lines.push(report.op_name().to_string());
        match report {
            Report::ReadUimlk(read) => {
                lines.push(format!("lock {}", read.state.lock_value));
                lines.push(format!("ctrl {}", read.state.control));
            }
            Report::Iccid(iccid) => {
                lines.push(format!("iccid {}", iccid.decoded_hex));
            }
            Report::Pin(pin) => {
                lines.push(format!("PIN {}", pin.lockcode));
            }
            Report::Status(status) => {
                if let Some(value) = status.lockcode.as_ref().and_then(|v| v.value()) {
                    lines.push(format!("PIN {value}"));
                }
                if let Some(lock) = &status.uim_lock {
                    lines.push(format!("lock {}", lock.lock_value));
                }
            }
            Report::Mutation(mutation) => {
                lines.push(format!("verified {}", mutation.verified));
            }
            Report::ResetPin(reset) => {
                lines.push(format!("PIN {}", reset.lockcode));
                lines.push(format!("verified {}", reset.verified));
            }
            Report::Maintenance(maintenance) => {
                lines.push(format!("bit {}", maintenance.maintenance_bit));
            }
            Report::Batch(batch) => {
                lines.push(format!("items {}", batch.results.len()));
            }
            Report::Probe(probe) => {
                lines.push(format!("{} {}", probe.probe.as_str(), probe.success));
            }
            Report::ExitUimlk(exit) => {
                lines.push(format!("ack {}", exit.success));
            }
            Report::ExitBase(exit) => {
                lines.push(format!("exit {}", exit.success));
            }
        }
        self.lines = lines;
    }

    fn apply_result_message(&mut self, kind: &str, message: &str) {
        self.screen = Screen::Result;
        self.lines.clear();
        self.lines.push(kind.to_string());
        for chunk in message.as_bytes().chunks(DISPLAY_COLS as usize) {
            self.lines.push(String::from_utf8_lossy(chunk).to_string());
        }
    }
}

/// Clip a line to the display width (the HAL also clips; kept as a single place
/// to add ellipsis/scroll behaviour for long error strings).
fn clip(text: &str) -> &str {
    text
}
