//! Navigation / confirmation-gating tests for the OLED UI (docs/plan/09 §5).
//!
//! These tests drive [`UiState`] directly (no protocol), which is exactly how
//! the platform input driver will drive it: one [`InputEvent`] at a time.

use auroku_core::Task;
use auroku_firmware::mock::MockDisplay;
use auroku_firmware::ui::{UiAction, UiState};
use auroku_firmware::InputEvent;

fn render(ui: &UiState) -> MockDisplay {
    let mut display = MockDisplay::new();
    ui.render(&mut display);
    display
}

fn rotate_to(ui: &mut UiState, entry: usize) {
    for _ in 0..entry {
        assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    }
}

#[test]
fn menu_covers_all_tasks_and_keeps_emu_navigation_order() {
    let mut ui = UiState::new();
    let display = render(&ui);
    assert!(
        display.line(0).contains("NO DEVICE"),
        "header: {}",
        display.line(0)
    );
    // Row 1 is the first entry; the emulator firmware rotates once to reach
    // "Read UIM lock" and selects it with a short press.
    assert!(
        display.line(1).contains("Status"),
        "row1: {}",
        display.line(1)
    );

    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(2).contains("Read UIM lock"));
    assert!(display.line(2).starts_with('>'));
    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::ReadUimlk)
    );
}

#[test]
fn top_level_short_press_on_write_opens_hex_editor() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 2); // Status, Read UIM lock, Write UIM lock
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(0).contains("WRITE UIM LOCK"));
    assert!(display.line(1).contains("VALUE 00000000"));
    assert!(display.line(2).contains("DIGIT 1 = 0"));
}

#[test]
fn writelk_editor_builds_value_and_control_then_requires_long_press() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 2);
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);

    // First digit -> 1, then walk to the control bit at the end.
    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    for _ in 0..8 {
        assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    }
    let display = render(&ui);
    assert!(
        display.line(2).contains("CONTROL = 0"),
        "row2: {}",
        display.line(2)
    );

    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(2).contains("CONTROL = 1"));

    // Short press finishes the editor and opens the confirmation page only.
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(0).contains("CONFIRM"));
    assert!(display.line(1).contains("WRITE UIM LOCK"));
    assert!(display.line(2).contains("value 10000000 ctl 1"));
    assert!(display.line(3).contains("NOT UNDOABLE"));

    // No short key path submits the write.
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    assert_eq!(ui.handle(InputEvent::EncoderCcw, 0), UiAction::None);

    // Long press confirms.
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::Writelk {
            value: "10000000".to_string(),
            control: 1,
        })
    );
}

#[test]
fn writelk_editor_back_walks_digits_then_returns_to_menu() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 2);
    ui.handle(InputEvent::EncoderPress, 0); // enter editor, cursor 0

    ui.handle(InputEvent::BackPress, 0); // cursor 0 -> leave editor
    let display = render(&ui);
    assert!(display.line(0).contains("auRokku"));

    // The menu selection is still on "Write UIM lock": re-enter the editor,
    // move to digit 2 and walk back to digit 1 without leaving the page.
    ui.handle(InputEvent::EncoderPress, 0);
    ui.handle(InputEvent::EncoderPress, 0); // cursor 1
    ui.handle(InputEvent::BackPress, 0); // cursor 0
    let display = render(&ui);
    assert!(display.line(0).contains("WRITE UIM LOCK"));
    assert!(display.line(2).contains("DIGIT 1"));
}

#[test]
fn destructive_clear_needs_long_press_and_back_cancels() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 3); // Clear UIM lock
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);

    let display = render(&ui);
    assert!(display.line(0).contains("CONFIRM"));
    assert!(display.line(3).contains("NOT UNDOABLE"));

    // Nothing but the long press may submit.
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    assert_eq!(ui.handle(InputEvent::EncoderCcw, 0), UiAction::None);

    assert_eq!(ui.handle(InputEvent::BackPress, 0), UiAction::None);
    let display = render(&ui);
    assert!(
        display.line(0).contains("auRokku"),
        "back must return to menu"
    );

    // Re-enter and confirm.
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::Clearlk)
    );
}

#[test]
fn pin_iccid_submenu_lists_and_submits_safe_operations() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 4); // PIN / ICCID
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);

    let display = render(&ui);
    assert!(display.line(0).contains("PIN / ICCID"));
    assert!(display.line(1).contains("Read PIN"));
    assert!(display.line(2).contains("Read ICCID"));
    assert!(display.line(3).contains("Reset PIN"));

    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::Readpin)
    );

    // Reset PIN stays behind the confirmation gate.
    let mut ui = UiState::new();
    rotate_to(&mut ui, 4);
    ui.handle(InputEvent::EncoderPress, 0);
    ui.handle(InputEvent::EncoderCw, 0);
    ui.handle(InputEvent::EncoderCw, 0);
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(0).contains("CONFIRM"));
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::Resetpin)
    );
}

#[test]
fn maintenance_submenu_offers_both_control_bits() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 5); // Maintenance
    ui.handle(InputEvent::EncoderPress, 0);

    let display = render(&ui);
    assert!(display.line(0).contains("MAINTENANCE"));
    assert!(display.line(1).contains("Set bit = 1"));

    ui.handle(InputEvent::EncoderPress, 0);
    let display = render(&ui);
    assert!(display.line(0).contains("CONFIRM"));
    assert!(display.line(2).contains("bit = 1"));
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::MaintenanceBit { control: 1 })
    );

    let mut ui = UiState::new();
    rotate_to(&mut ui, 5);
    ui.handle(InputEvent::EncoderPress, 0);
    ui.handle(InputEvent::EncoderCw, 0);
    ui.handle(InputEvent::EncoderPress, 0);
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::MaintenanceBit { control: 0 })
    );
}

#[test]
fn probe_submenu_covers_kinds_and_cycles_timeout() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 6); // Probe
    ui.handle(InputEvent::EncoderPress, 0);

    let display = render(&ui);
    assert!(display.line(0).contains("PROBE"));
    assert!(display.line(1).contains("SERI echo"));
    assert!(display.line(2).contains("SCDP echo"));
    assert!(display.line(3).contains("Timeout 1s"));

    // Queue a timeout change (no submission).
    ui.handle(InputEvent::EncoderCw, 0);
    ui.handle(InputEvent::EncoderCw, 0);
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    let display = render(&ui);
    assert!(display.line(3).contains("Timeout 2s"));

    // SERI probe with the selected timeout.
    ui.handle(InputEvent::EncoderCcw, 0);
    ui.handle(InputEvent::EncoderCcw, 0);
    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::Probe {
            kind: auroku_core::ProbeKind::Seri,
            timeout_ms: 2000,
        })
    );
}

#[test]
fn batch_submenu_submits_both_profiles() {
    for (entry_cw, expected) in [
        (0, auroku_core::BatchProfile::Basic),
        (1, auroku_core::BatchProfile::Uimlk),
    ] {
        let mut ui = UiState::new();
        rotate_to(&mut ui, 7); // Batch
        ui.handle(InputEvent::EncoderPress, 0);
        for _ in 0..entry_cw {
            ui.handle(InputEvent::EncoderCw, 0);
        }
        assert_eq!(
            ui.handle(InputEvent::EncoderPress, 0),
            UiAction::Submit(Task::Batch { profile: expected })
        );
    }
}

#[test]
fn session_submenu_keeps_exit_base_safe_and_exit_uimlk_gated() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 8); // Session
    ui.handle(InputEvent::EncoderPress, 0);

    let display = render(&ui);
    assert!(display.line(0).contains("SESSION"));
    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::Exit)
    );

    let mut ui = UiState::new();
    rotate_to(&mut ui, 8);
    ui.handle(InputEvent::EncoderPress, 0);
    ui.handle(InputEvent::EncoderCw, 0);
    assert_eq!(ui.handle(InputEvent::EncoderPress, 0), UiAction::None);
    assert_eq!(
        ui.handle(InputEvent::EncoderLongPress, 0),
        UiAction::Submit(Task::Exituimlk)
    );
}

#[test]
fn back_from_submenu_returns_to_main_menu() {
    let mut ui = UiState::new();
    rotate_to(&mut ui, 4);
    ui.handle(InputEvent::EncoderPress, 0);
    assert!(render(&ui).line(0).contains("PIN / ICCID"));
    assert_eq!(ui.handle(InputEvent::BackPress, 0), UiAction::None);
    assert!(render(&ui).line(0).contains("auRokku"));
}

#[test]
fn long_press_on_menu_or_submenu_does_not_submit() {
    let mut ui = UiState::new();
    assert_eq!(ui.handle(InputEvent::EncoderLongPress, 0), UiAction::None);
    rotate_to(&mut ui, 4);
    ui.handle(InputEvent::EncoderPress, 0);
    assert_eq!(ui.handle(InputEvent::EncoderLongPress, 0), UiAction::None);
}
