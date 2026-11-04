//! OLED menu UI: pure state machine over [`Display`] + [`InputEvent`].
//!
//! Implements the interaction design of docs/plan/09:
//! - a menu covering all 12 [`Task`]s, with submenus for PIN/ICCID, maintenance,
//!   probe, batch and session operations;
//! - parameter editing for `writelk` (8 hex digits + control bit) and for the
//!   probe timeout / batch profile / maintenance control;
//! - destructive operations (write/clear UIM lock, reset PIN, maintenance bit,
//!   exit UIMLK) only ever run from the long-press confirmation page: a short
//!   press on a menu item only opens that page, and BACK cancels it;
//! - typed result pages for every [`Report`] variant, with paging on the 21x6
//!   character grid (ICCID/raw hex, long error messages);
//! - error pages mapped from `ValueError` / `RuntimeError` / `TimeoutError` /
//!   `DriverError`, plus the UI-level `NO DEVICE` page and detach reset.
//!
//! The UI never talks to USB: [`crate::app`] turns [`UiAction`]s into protocol
//! calls and feeds outcomes back through [`UiState::on_outcome`].
//!
//! Menu layout note (raster contract): the top-level order starts with
//! `Status`, `Read UIM lock`, `Write UIM lock`, `Clear UIM lock`. The
//! `firmware/rp2354-emu-fw` scenarios drive the UI with one/few encoder clicks
//! from the first entry, so these four stay top-level; the remaining operations
//! are grouped in submenus.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use auroku_core::report::{
    BatchReport, BatchResult, ExitBaseReport, ExitOutcome, FieldValue, IccidReport,
    MaintenanceReport, MutationReport, ProbeReport, ReadUimlkReport, Report, ResetPinReport,
    StatusReport,
};
use auroku_core::{BatchProfile, ProbeKind, Task};

use crate::hal::{Display, InputEvent, DISPLAY_COLS, DISPLAY_ROWS};
use crate::protocol::{DeviceInfo, ProtoOutcome};

const ROWS: usize = DISPLAY_ROWS as usize;
const COLS: usize = DISPLAY_COLS as usize;
/// Rows 1..=4 are the list/content area; row 0 is the title, row 5 the footer.
const CONTENT_ROWS: usize = 4;

/// What the UI asks the app to do after an input event.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    /// Nothing to do (navigation, redraw, ...).
    None,
    /// Run this task against the attached phone.
    Submit(Task),
}

/// Top-level menu entries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MainEntry {
    Status,
    ReadUimlk,
    WriteUimlk,
    ClearUimlk,
    PinIccid,
    Maintenance,
    Probe,
    Batch,
    Session,
}

/// Top-level menu order. The first four entries are pinned by the emulator
/// firmware scenarios (docs/plan/13) and must keep their positions.
const MAIN: [MainEntry; 9] = [
    MainEntry::Status,
    MainEntry::ReadUimlk,
    MainEntry::WriteUimlk,
    MainEntry::ClearUimlk,
    MainEntry::PinIccid,
    MainEntry::Maintenance,
    MainEntry::Probe,
    MainEntry::Batch,
    MainEntry::Session,
];

impl MainEntry {
    fn label(self) -> &'static str {
        match self {
            MainEntry::Status => "Status",
            MainEntry::ReadUimlk => "Read UIM lock",
            MainEntry::WriteUimlk => "Write UIM lock",
            MainEntry::ClearUimlk => "Clear UIM lock",
            MainEntry::PinIccid => "PIN / ICCID",
            MainEntry::Maintenance => "Maintenance",
            MainEntry::Probe => "Probe",
            MainEntry::Batch => "Batch",
            MainEntry::Session => "Session",
        }
    }

    fn submenu(self) -> Option<Submenu> {
        match self {
            MainEntry::PinIccid => Some(Submenu::PinIccid),
            MainEntry::Maintenance => Some(Submenu::Maintenance),
            MainEntry::Probe => Some(Submenu::Probe),
            MainEntry::Batch => Some(Submenu::Batch),
            MainEntry::Session => Some(Submenu::Session),
            _ => None,
        }
    }
}

/// Grouped secondary menus.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Submenu {
    PinIccid,
    Maintenance,
    Probe,
    Batch,
    Session,
}

const PIN_ICCID_ITEMS: [SubItem; 3] = [SubItem::ReadPin, SubItem::ReadIccid, SubItem::ResetPin];
const MAINTENANCE_ITEMS: [SubItem; 2] = [SubItem::Maintenance1, SubItem::Maintenance0];
const PROBE_ITEMS: [SubItem; 3] = [
    SubItem::ProbeSeri,
    SubItem::ProbeScdp,
    SubItem::ProbeTimeout,
];
const BATCH_ITEMS: [SubItem; 2] = [SubItem::BatchBasic, SubItem::BatchUimlk];
const SESSION_ITEMS: [SubItem; 2] = [SubItem::ExitBase, SubItem::ExitUimlk];

impl Submenu {
    fn title(self) -> &'static str {
        match self {
            Submenu::PinIccid => "PIN / ICCID",
            Submenu::Maintenance => "MAINTENANCE",
            Submenu::Probe => "PROBE",
            Submenu::Batch => "BATCH",
            Submenu::Session => "SESSION",
        }
    }

    fn items(self) -> &'static [SubItem] {
        match self {
            Submenu::PinIccid => &PIN_ICCID_ITEMS,
            Submenu::Maintenance => &MAINTENANCE_ITEMS,
            Submenu::Probe => &PROBE_ITEMS,
            Submenu::Batch => &BATCH_ITEMS,
            Submenu::Session => &SESSION_ITEMS,
        }
    }
}

/// One entry inside a submenu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SubItem {
    ReadPin,
    ReadIccid,
    ResetPin,
    Maintenance1,
    Maintenance0,
    ProbeSeri,
    ProbeScdp,
    ProbeTimeout,
    BatchBasic,
    BatchUimlk,
    ExitBase,
    ExitUimlk,
}

impl SubItem {
    fn label(self, probe_timeout_ms: u32) -> String {
        match self {
            SubItem::ReadPin => "Read PIN".to_string(),
            SubItem::ReadIccid => "Read ICCID".to_string(),
            SubItem::ResetPin => "Reset PIN".to_string(),
            SubItem::Maintenance1 => "Set bit = 1".to_string(),
            SubItem::Maintenance0 => "Set bit = 0".to_string(),
            SubItem::ProbeSeri => "SERI echo".to_string(),
            SubItem::ProbeScdp => "SCDP echo".to_string(),
            SubItem::ProbeTimeout => format!("Timeout {}s", probe_timeout_ms / 1000),
            SubItem::BatchBasic => "Profile basic".to_string(),
            SubItem::BatchUimlk => "Profile uimlk".to_string(),
            SubItem::ExitBase => "Exit base 0xD8".to_string(),
            SubItem::ExitUimlk => "Exit UIMLK 0x56".to_string(),
        }
    }
}

/// Operations that require the long-press confirmation page.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pending {
    Writelk { value: String, control: u8 },
    Clearlk,
    Resetpin,
    Maintenance { control: u8 },
    Exituimlk,
}

impl Pending {
    fn task(&self) -> Task {
        match self {
            Pending::Writelk { value, control } => Task::Writelk {
                value: value.clone(),
                control: *control,
            },
            Pending::Clearlk => Task::Clearlk,
            Pending::Resetpin => Task::Resetpin,
            Pending::Maintenance { control } => Task::MaintenanceBit { control: *control },
            Pending::Exituimlk => Task::Exituimlk,
        }
    }

    fn title(&self) -> &'static str {
        match self {
            Pending::Writelk { .. } => "WRITE UIM LOCK",
            Pending::Clearlk => "CLEAR UIM LOCK",
            Pending::Resetpin => "RESET PIN",
            Pending::Maintenance { .. } => "MAINTENANCE BIT",
            Pending::Exituimlk => "EXIT UIMLK",
        }
    }

    fn detail(&self) -> String {
        match self {
            Pending::Writelk { value, control } => {
                format!("value {value} ctl {control}")
            }
            Pending::Clearlk => "new value FFFFFFFF".to_string(),
            Pending::Resetpin => "PIN -> 1234".to_string(),
            Pending::Maintenance { control } => format!("bit = {control}, may reboot"),
            Pending::Exituimlk => "phone may reboot".to_string(),
        }
    }
}

/// Window state of the 8 hex digit + control bit editor.
#[derive(Clone, Debug, Default)]
struct WriteEdit {
    digits: [u8; 8],
    /// 0..=7 selects a hex digit, 8 selects the control bit.
    cursor: usize,
    control: u8,
}

impl WriteEdit {
    fn value_hex(&self) -> String {
        self.digits.iter().map(|&d| hex_digit(d)).collect()
    }

    fn control(&self) -> u8 {
        self.control.min(1)
    }

    fn bump(&mut self, up: bool) {
        if self.cursor < 8 {
            let digit = &mut self.digits[self.cursor];
            *digit = if up {
                (*digit + 1) & 0x0F
            } else {
                (*digit + 0x0F) & 0x0F
            };
        } else {
            // Either rotation direction toggles the control bit.
            self.control ^= 1;
        }
    }
}

fn hex_digit(digit: u8) -> char {
    match digit & 0x0F {
        0..=9 => (b'0' + (digit & 0x0F)) as char,
        other => (b'A' + (other - 10)) as char,
    }
}

/// How the result page was produced (affects the footer action).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ResultKind {
    Report,
    Error,
    NoDevice,
}

/// A paged result/error screen.
#[derive(Clone, Debug)]
struct ResultPage {
    title: String,
    /// Body lines; four per page (rows 1..=4).
    lines: Vec<String>,
    page: usize,
    kind: ResultKind,
}

impl ResultPage {
    fn pages(&self) -> usize {
        if self.lines.is_empty() {
            1
        } else {
            self.lines.len().div_ceil(CONTENT_ROWS)
        }
    }

    fn page_lines(&self) -> &[String] {
        let start = (self.page.saturating_mul(CONTENT_ROWS)).min(self.lines.len());
        let end = (start + CONTENT_ROWS).min(self.lines.len());
        &self.lines[start..end]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Submenu,
    WriteEdit,
    Confirm,
    Result,
}

/// The OLED UI state machine.
pub struct UiState {
    screen: Screen,
    menu_index: usize,
    submenu: Option<Submenu>,
    sub_index: usize,
    probe_timeout_ms: u32,
    edit: WriteEdit,
    pending: Option<Pending>,
    device: Option<DeviceInfo>,
    result: Option<ResultPage>,
    last_task: Option<Task>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            screen: Screen::Menu,
            menu_index: 0,
            submenu: None,
            sub_index: 0,
            probe_timeout_ms: 1000,
            edit: WriteEdit::default(),
            pending: None,
            device: None,
            result: None,
            last_task: None,
        }
    }

    /// Platform callback: a phone is attached and configured.
    pub fn on_attach(&mut self, device: DeviceInfo) {
        self.device = Some(device);
        self.reset_to_menu();
    }

    /// Platform callback: the phone is gone (unplug, bus reset, ...).
    pub fn on_detach(&mut self) {
        self.device = None;
        self.reset_to_menu();
    }

    /// There is no attached phone, so a submitted task cannot run.
    pub fn on_no_device(&mut self) {
        self.set_result(
            "NO DEVICE".to_string(),
            vec!["attach phone".to_string(), "then retry".to_string()],
            ResultKind::NoDevice,
        );
    }

    /// Protocol callback: render the typed outcome.
    pub fn on_outcome(&mut self, outcome: &ProtoOutcome) {
        match &outcome.result {
            Ok(task_outcome) => {
                let mut lines = report_lines(&task_outcome.report);
                if let Some(exit) = &task_outcome.exit {
                    lines.extend(exit_lines(exit));
                }
                self.set_result(
                    task_outcome.report.op_name().to_string(),
                    lines,
                    ResultKind::Report,
                );
            }
            Err(error) => self.set_error(error.kind(), &error.to_string()),
        }
    }

    /// Handle one input event. `now_ms` is unused: the platform input driver
    /// emits [`InputEvent::EncoderLongPress`] once the 1.5 s threshold is
    /// crossed (docs/plan/09 §4.3), so the UI only interprets the event.
    pub fn handle(&mut self, event: InputEvent, _now_ms: u64) -> UiAction {
        match (self.screen, event) {
            (Screen::Menu, InputEvent::EncoderCw) => {
                self.menu_index = (self.menu_index + 1) % MAIN.len();
                UiAction::None
            }
            (Screen::Menu, InputEvent::EncoderCcw) => {
                self.menu_index = (self.menu_index + MAIN.len() - 1) % MAIN.len();
                UiAction::None
            }
            (Screen::Menu, InputEvent::EncoderPress) => self.activate_menu(),
            (Screen::Menu, _) => UiAction::None,

            (Screen::Submenu, InputEvent::EncoderCw) => {
                self.sub_index = (self.sub_index + 1) % self.submenu_len();
                UiAction::None
            }
            (Screen::Submenu, InputEvent::EncoderCcw) => {
                self.sub_index = (self.sub_index + self.submenu_len() - 1) % self.submenu_len();
                UiAction::None
            }
            (Screen::Submenu, InputEvent::EncoderPress) => self.activate_submenu(),
            (Screen::Submenu, InputEvent::BackPress) => {
                self.screen = Screen::Menu;
                UiAction::None
            }
            (Screen::Submenu, _) => UiAction::None,

            (Screen::WriteEdit, InputEvent::EncoderCw) => {
                self.edit.bump(true);
                UiAction::None
            }
            (Screen::WriteEdit, InputEvent::EncoderCcw) => {
                self.edit.bump(false);
                UiAction::None
            }
            (Screen::WriteEdit, InputEvent::EncoderPress) => self.edit_advance(),
            (Screen::WriteEdit, InputEvent::BackPress) => {
                if self.edit.cursor > 0 {
                    self.edit.cursor -= 1;
                    UiAction::None
                } else {
                    self.screen = Screen::Menu;
                    UiAction::None
                }
            }
            (Screen::WriteEdit, _) => UiAction::None,

            (Screen::Confirm, InputEvent::EncoderLongPress) => match self.pending.take() {
                Some(pending) => self.submit(pending.task()),
                None => UiAction::None,
            },
            (Screen::Confirm, InputEvent::BackPress) => {
                self.pending = None;
                self.screen = Screen::Menu;
                UiAction::None
            }
            // A short press / rotation must never confirm a destructive op.
            (Screen::Confirm, _) => UiAction::None,

            (Screen::Result, InputEvent::EncoderCw | InputEvent::EncoderPress) => {
                self.result_next()
            }
            (Screen::Result, InputEvent::EncoderCcw) => {
                self.result_prev();
                UiAction::None
            }
            (Screen::Result, InputEvent::BackPress) => {
                self.screen = Screen::Menu;
                UiAction::None
            }
            (Screen::Result, _) => UiAction::None,
        }
    }

    /// Paint the current screen through the HAL.
    pub fn render(&self, display: &mut impl Display) {
        display.clear();
        match self.screen {
            Screen::Menu => self.render_menu(display),
            Screen::Submenu => self.render_submenu(display),
            Screen::WriteEdit => self.render_write_edit(display),
            Screen::Confirm => self.render_confirm(display),
            Screen::Result => self.render_result(display),
        }
    }

    // ------------------------------------------------------------------
    // Transitions
    // ------------------------------------------------------------------

    fn reset_to_menu(&mut self) {
        self.screen = Screen::Menu;
        self.menu_index = 0;
        self.submenu = None;
        self.sub_index = 0;
        self.edit = WriteEdit::default();
        self.pending = None;
        self.result = None;
    }

    fn submenu_len(&self) -> usize {
        self.submenu.map(|sub| sub.items().len()).unwrap_or(1)
    }

    fn activate_menu(&mut self) -> UiAction {
        match MAIN[self.menu_index] {
            MainEntry::Status => self.submit(Task::Status),
            MainEntry::ReadUimlk => self.submit(Task::ReadUimlk),
            MainEntry::WriteUimlk => {
                self.edit = WriteEdit::default();
                self.screen = Screen::WriteEdit;
                UiAction::None
            }
            MainEntry::ClearUimlk => {
                self.pending = Some(Pending::Clearlk);
                self.screen = Screen::Confirm;
                UiAction::None
            }
            entry => {
                if let Some(submenu) = entry.submenu() {
                    self.submenu = Some(submenu);
                    self.sub_index = 0;
                    self.screen = Screen::Submenu;
                }
                UiAction::None
            }
        }
    }

    fn activate_submenu(&mut self) -> UiAction {
        let Some(submenu) = self.submenu else {
            return UiAction::None;
        };
        let items = submenu.items();
        let Some(item) = items.get(self.sub_index).copied() else {
            return UiAction::None;
        };
        match item {
            SubItem::ReadPin => self.submit(Task::Readpin),
            SubItem::ReadIccid => self.submit(Task::ReadIccid),
            SubItem::ProbeSeri => self.submit(Task::Probe {
                kind: ProbeKind::Seri,
                timeout_ms: self.probe_timeout_ms,
            }),
            SubItem::ProbeScdp => self.submit(Task::Probe {
                kind: ProbeKind::Scdp,
                timeout_ms: self.probe_timeout_ms,
            }),
            SubItem::BatchBasic => self.submit(Task::Batch {
                profile: BatchProfile::Basic,
            }),
            SubItem::BatchUimlk => self.submit(Task::Batch {
                profile: BatchProfile::Uimlk,
            }),
            SubItem::ExitBase => self.submit(Task::Exit),
            SubItem::ResetPin => {
                self.pending = Some(Pending::Resetpin);
                self.screen = Screen::Confirm;
                UiAction::None
            }
            SubItem::Maintenance1 => {
                self.pending = Some(Pending::Maintenance { control: 1 });
                self.screen = Screen::Confirm;
                UiAction::None
            }
            SubItem::Maintenance0 => {
                self.pending = Some(Pending::Maintenance { control: 0 });
                self.screen = Screen::Confirm;
                UiAction::None
            }
            SubItem::ExitUimlk => {
                self.pending = Some(Pending::Exituimlk);
                self.screen = Screen::Confirm;
                UiAction::None
            }
            SubItem::ProbeTimeout => {
                self.probe_timeout_ms = match self.probe_timeout_ms {
                    1000 => 2000,
                    2000 => 3000,
                    _ => 1000,
                };
                UiAction::None
            }
        }
    }

    fn edit_advance(&mut self) -> UiAction {
        if self.edit.cursor < 8 {
            self.edit.cursor += 1;
            UiAction::None
        } else {
            self.pending = Some(Pending::Writelk {
                value: self.edit.value_hex(),
                control: self.edit.control(),
            });
            self.screen = Screen::Confirm;
            UiAction::None
        }
    }

    fn submit(&mut self, task: Task) -> UiAction {
        self.last_task = Some(task.clone());
        self.set_result(
            task.op_name().to_string(),
            vec!["working...".to_string()],
            ResultKind::Report,
        );
        UiAction::Submit(task)
    }

    fn result_next(&mut self) -> UiAction {
        let Some(result) = self.result.as_mut() else {
            return UiAction::None;
        };
        let pages = result.pages();
        if pages > 1 && result.page + 1 < pages {
            result.page += 1;
            return UiAction::None;
        }
        // Single page (or last page): OK retries a failed operation, otherwise
        // it leaves the result page like BACK.
        if result.kind == ResultKind::Error {
            if let Some(task) = self.last_task.clone() {
                return self.submit(task);
            }
        }
        self.screen = Screen::Menu;
        UiAction::None
    }

    fn result_prev(&mut self) {
        if let Some(result) = self.result.as_mut() {
            let pages = result.pages();
            if pages > 1 {
                result.page = if result.page == 0 {
                    pages - 1
                } else {
                    result.page - 1
                };
            }
        }
    }

    fn set_result(&mut self, title: String, mut lines: Vec<String>, kind: ResultKind) {
        if lines.is_empty() {
            lines.push("(no data)".to_string());
        }
        self.result = Some(ResultPage {
            title,
            lines,
            page: 0,
            kind,
        });
        self.screen = Screen::Result;
    }

    fn set_error(&mut self, kind: &'static str, message: &str) {
        let mut lines = Vec::new();
        push_prefixed(&mut lines, "", message);
        push_prefixed(&mut lines, "", error_hint(kind));
        self.set_result(format!("ERROR {kind}"), lines, ResultKind::Error);
    }

    // ------------------------------------------------------------------
    // Rendering
    // ------------------------------------------------------------------

    fn render_menu(&self, display: &mut impl Display) {
        let header = match &self.device {
            Some(device) => match &device.sn {
                Some(sn) => format!("auRokku {sn}"),
                None => "auRokku USB".to_string(),
            },
            None => "auRokku NO DEVICE".to_string(),
        };
        write_line(display, 0, &header);

        let start = window_start(self.menu_index, MAIN.len());
        for (row, entry) in MAIN.iter().skip(start).take(CONTENT_ROWS).enumerate() {
            let index = start + row;
            let marker = if index == self.menu_index { '>' } else { ' ' };
            write_line(
                display,
                1 + row as u8,
                &format!("{marker}{}", entry.label()),
            );
        }
        write_line(
            display,
            5,
            &format!("{}/{} OK=enter BACK=up", self.menu_index + 1, MAIN.len()),
        );
    }

    fn render_submenu(&self, display: &mut impl Display) {
        let Some(submenu) = self.submenu else {
            return;
        };
        write_line(display, 0, submenu.title());
        let items = submenu.items();
        let start = window_start(self.sub_index, items.len());
        for (row, item) in items.iter().skip(start).take(CONTENT_ROWS).enumerate() {
            let index = start + row;
            let marker = if index == self.sub_index { '>' } else { ' ' };
            write_line(
                display,
                1 + row as u8,
                &format!("{marker}{}", item.label(self.probe_timeout_ms)),
            );
        }
        write_line(
            display,
            5,
            &format!("{}/{} OK=enter BACK=up", self.sub_index + 1, items.len()),
        );
    }

    fn render_write_edit(&self, display: &mut impl Display) {
        write_line(display, 0, "WRITE UIM LOCK");
        write_line(display, 1, &format!("VALUE {}", self.edit.value_hex()));
        if self.edit.cursor < 8 {
            let digit = self.edit.digits[self.edit.cursor];
            write_line(
                display,
                2,
                &format!("DIGIT {} = {}", self.edit.cursor + 1, hex_digit(digit)),
            );
        } else {
            write_line(display, 2, &format!("CONTROL = {}", self.edit.control()));
        }
        write_line(display, 3, "ROT=change OK=next");
        write_line(display, 4, "BACK=previous");
        write_line(display, 5, "4 bytes, 8 hex digits");
    }

    fn render_confirm(&self, display: &mut impl Display) {
        let (title, detail) = match &self.pending {
            Some(pending) => (pending.title(), pending.detail()),
            None => ("UNKNOWN", String::new()),
        };
        write_line(display, 0, "CONFIRM");
        write_line(display, 1, title);
        write_line(display, 2, &detail);
        write_line(display, 3, "NOT UNDOABLE");
        write_line(display, 4, "Hold OK 1.5s");
        write_line(display, 5, "BACK = cancel");
    }

    fn render_result(&self, display: &mut impl Display) {
        let Some(result) = self.result.as_ref() else {
            write_line(display, 0, "working...");
            return;
        };
        write_line(display, 0, &result.title);
        for (row, line) in result.page_lines().iter().enumerate() {
            write_line(display, 1 + row as u8, line);
        }
        let pages = result.pages();
        let footer = if pages > 1 {
            format!("{}/{} OK=next BACK=up", result.page + 1, pages)
        } else {
            match result.kind {
                ResultKind::Error => "OK=retry BACK=up".to_string(),
                ResultKind::Report | ResultKind::NoDevice => "BACK=menu".to_string(),
            }
        };
        write_line(display, (ROWS - 1) as u8, &footer);
    }
}

// ----------------------------------------------------------------------
// Result page builders
// ----------------------------------------------------------------------

fn report_lines(report: &Report) -> Vec<String> {
    match report {
        Report::Status(status) => status_lines(status),
        Report::ReadUimlk(read) => read_uimlk_lines(read),
        Report::Mutation(mutation) => mutation_lines(mutation),
        Report::Iccid(iccid) => iccid_lines(iccid),
        Report::Pin(pin) => vec![
            format!("PIN {}", pin.lockcode),
            format!("raw {}", pin.raw_hex),
        ],
        Report::ResetPin(reset) => reset_pin_lines(reset),
        Report::Maintenance(maintenance) => maintenance_lines(maintenance),
        Report::Batch(batch) => batch_lines(batch),
        Report::Probe(probe) => probe_lines(probe),
        Report::ExitUimlk(exit) => vec![
            format!("ack {}", yes_no(exit.success)),
            "phone may reboot".to_string(),
            format!("reply {}", exit.reply),
        ],
        Report::ExitBase(exit) => exit_base_lines(exit),
    }
}

fn status_lines(status: &StatusReport) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("sn {}", status.sn.as_deref().unwrap_or("-")));
    push_field(&mut lines, "ver ", status.version.as_ref());
    push_field(&mut lines, "PIN ", status.lockcode.as_ref());
    match &status.uim_lock {
        Some(lock) => lines.push(format!(
            "UIM {} ctl {}",
            fmt_lock(&lock.lock_value),
            lock.control
        )),
        None => lines.push("UIM -".to_string()),
    }
    push_field(&mut lines, "ICCID ", status.iccid_record.as_ref());
    lines
}

fn read_uimlk_lines(read: &ReadUimlkReport) -> Vec<String> {
    let state = &read.state;
    let mut lines = Vec::new();
    // First three lines are part of the emulator firmware contract
    // (docs/plan/13): op name, raw lock value, control bit.
    lines.push(format!("lock {}", state.lock_value));
    lines.push(format!(
        "ctrl {} {}",
        state.control,
        if state.binding_cleared {
            "open"
        } else {
            "bound"
        }
    ));
    lines.push(format!(
        "binding {}",
        if state.binding_cleared {
            "clear"
        } else {
            "set"
        }
    ));
    push_prefixed(&mut lines, "raw ", &state.raw_hex);
    lines
}

fn mutation_lines(mutation: &MutationReport) -> Vec<String> {
    let mut lines = Vec::new();
    push_prefixed(
        &mut lines,
        "before ",
        &fmt_lock(&mutation.before.lock_value),
    );
    match &mutation.after {
        Some(after) => push_prefixed(&mut lines, "after  ", &fmt_lock(&after.lock_value)),
        None => lines.push("after  ?".to_string()),
    }
    lines.push(if mutation.verified {
        "verified OK".to_string()
    } else {
        "VERIFY NG".to_string()
    });
    if mutation.partial_write_possible {
        lines.push("PARTIAL WRITE POSS.".to_string());
    }
    lines.push(format!(
        "cmd {}",
        if mutation.command_accepted {
            "OK"
        } else {
            "NG"
        }
    ));
    if let Some(reply) = &mutation.reply {
        push_prefixed(&mut lines, "reply ", reply);
    }
    if let Some(error) = &mutation.command_error {
        push_prefixed(&mut lines, "cmd err ", error);
    }
    if let Some(error) = &mutation.readback_error {
        push_prefixed(&mut lines, "read err ", error);
    }
    lines
}

fn iccid_lines(iccid: &IccidReport) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("raw:".to_string());
    push_prefixed(&mut lines, "", &iccid.raw_hex);
    lines.push("decoded:".to_string());
    push_prefixed(&mut lines, "", &iccid.decoded_hex);
    lines
}

fn reset_pin_lines(reset: &ResetPinReport) -> Vec<String> {
    vec![
        format!("PIN {}", reset.lockcode),
        format!("verified {}", yes_no(reset.verified)),
        format!("reply {}", reset.reply),
    ]
}

fn maintenance_lines(maintenance: &MaintenanceReport) -> Vec<String> {
    vec![
        format!("bit {}", maintenance.maintenance_bit),
        format!("accepted {}", yes_no(maintenance.command_accepted)),
        format!("success {}", yes_no(maintenance.success)),
        format!("reply {}", maintenance.reply),
    ]
}

fn batch_lines(batch: &BatchReport) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("profile {}", batch.profile.as_str()));
    lines.push(format!("success {}", yes_no(batch.success)));
    for item in &batch.results {
        match item {
            BatchResult::Version { raw_hex } => push_prefixed(&mut lines, "[OK] v ", raw_hex),
            BatchResult::Lockcode { raw_hex, decoded } => match decoded {
                Some(decoded) => push_prefixed(&mut lines, "[OK] pin ", decoded),
                None => push_prefixed(&mut lines, "[OK] raw ", raw_hex),
            },
            BatchResult::State(state) => {
                lines.push(format!("[OK] lock {}", fmt_lock(&state.lock_value)))
            }
            BatchResult::Record59 { decoded_hex, .. } => {
                push_prefixed(&mut lines, "[OK] iccid ", decoded_hex)
            }
        }
    }
    if let Some(error) = &batch.error {
        push_prefixed(
            &mut lines,
            "[NG] ",
            &format!("{}: {}", error.kind, error.message),
        );
    }
    lines
}

fn probe_lines(probe: &ProbeReport) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("kind {}", probe.probe.as_str()));
    if let Some(matched) = probe.matched {
        lines.push(format!("matched {}", yes_no(matched)));
    }
    lines.push(format!("resp {}", yes_no(probe.responsive)));
    if let Some(request) = &probe.request_hex {
        push_prefixed(&mut lines, "req ", request);
    }
    if let Some(response) = &probe.response_hex {
        push_prefixed(&mut lines, "resp ", response);
    }
    if let Some(cleanup) = &probe.cleanup_hex {
        push_prefixed(&mut lines, "cleanup ", cleanup);
    }
    if let Some(length) = probe.response_length {
        lines.push(format!("len {length}"));
    }
    lines
}

fn exit_base_lines(exit: &ExitBaseReport) -> Vec<String> {
    vec![
        format!("method {}", exit.method),
        format!("success {}", yes_no(exit.success)),
    ]
}

fn exit_lines(exit: &ExitOutcome) -> Vec<String> {
    let mut lines = Vec::new();
    if exit.acknowledged {
        lines.push("exit ack OK".to_string());
        if let Some(reply) = &exit.reply {
            lines.push(format!("exit reply {reply}"));
        }
    } else {
        lines.push("exit NO ACK".to_string());
        if let Some(error) = &exit.error {
            push_prefixed(&mut lines, "", error);
        }
    }
    if exit.may_reboot {
        lines.push("may reboot".to_string());
    }
    lines
}

fn push_field(lines: &mut Vec<String>, prefix: &str, field: Option<&FieldValue<String>>) {
    match field {
        Some(FieldValue::Value(value)) => push_prefixed(lines, prefix, value),
        Some(FieldValue::Error(message)) => push_prefixed(lines, prefix, message),
        None => {}
    }
}

fn error_hint(kind: &str) -> &'static str {
    match kind {
        "ValueError" => "check 8 hex digits, ctl 0-1",
        "RuntimeError" => "device rejected cmd",
        "TimeoutError" => "no answer, check cable",
        "DriverError" => "USB host error, check power",
        _ => "check cable and retry",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "OK"
    } else {
        "NG"
    }
}

/// Format an 8-hex lock value as two groups of four uppercase chars
/// (`80E4 8ED2`), matching docs/plan/09 §5.4. Values of other lengths are
/// upper-cased but left as-is.
fn fmt_lock(value: &str) -> String {
    if value.len() == 8 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        format!(
            "{} {}",
            value[..4].to_uppercase(),
            value[4..].to_uppercase()
        )
    } else {
        value.to_uppercase()
    }
}

/// Append `text` to `lines`, wrapping at the display width and keeping `prefix`
/// on the first line.
fn push_prefixed(lines: &mut Vec<String>, prefix: &str, text: &str) {
    let mut line = String::from(prefix);
    for ch in text.chars() {
        if line.chars().count() >= COLS {
            lines.push(line);
            line = String::new();
        }
        line.push(if ch.is_ascii() { ch } else { '?' });
    }
    lines.push(line);
}

fn write_line(display: &mut impl Display, row: u8, text: &str) {
    display.text(row, 0, &fit(text));
}

/// Truncate to the 21-character grid (the HAL implementation also clips).
fn fit(text: &str) -> String {
    text.chars()
        .take(COLS)
        .map(|ch| if ch.is_ascii() { ch } else { '?' })
        .collect()
}

/// First visible item of a scrolling window that keeps `selected` in view.
fn window_start(selected: usize, len: usize) -> usize {
    if len <= CONTENT_ROWS {
        return 0;
    }
    let mut start = selected.saturating_sub(CONTENT_ROWS - 1);
    if start + CONTENT_ROWS > len {
        start = len - CONTENT_ROWS;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockDisplay;

    #[test]
    fn window_keeps_selection_visible() {
        assert_eq!(window_start(0, 9), 0);
        assert_eq!(window_start(3, 9), 0);
        assert_eq!(window_start(4, 9), 1);
        assert_eq!(window_start(8, 9), 5);
        assert_eq!(window_start(1, 3), 0);
    }

    #[test]
    fn lock_value_formatting() {
        assert_eq!(fmt_lock("80e48ed2"), "80E4 8ED2");
        assert_eq!(fmt_lock("FFFFFFFF"), "FFFF FFFF");
        assert_eq!(fmt_lock("abcd"), "ABCD");
    }

    #[test]
    fn writelk_editor_produces_hex_value() {
        let mut edit = WriteEdit::default();
        edit.bump(true);
        assert_eq!(edit.value_hex(), "10000000");
        edit.cursor = 8;
        edit.bump(true);
        assert_eq!(edit.control(), 1);
        assert_eq!(hex_digit(0x0A), 'A');
        assert_eq!(hex_digit(0x0F), 'F');
    }

    #[test]
    fn menu_short_press_on_destructive_entry_opens_confirm_only() {
        let mut ui = UiState::new();
        let mut display = MockDisplay::new();
        // Clear UIM lock is the 4th top-level entry (3 rotations).
        for _ in 0..3 {
            assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
        }
        assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
        ui.render(&mut display);
        assert!(display.line(0).contains("CONFIRM"));
        assert!(display.line(3).contains("NOT UNDOABLE"));
    }
}
