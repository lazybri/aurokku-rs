//! End-to-end UI + protocol tests through [`App`] with the mock transport.
//!
//! These mirror the `rp2350-emu` firmware scenarios on the host: the same
//! encoder input drives the real `UiState`, the real `auroku-core` session runs
//! against a scripted phone, and the rendered 21x6 grid is asserted.

use auroku_core::mock::MockTransport;
use auroku_firmware::mock::{MockClock, MockDisplay, MockInput};
use auroku_firmware::{App, AppConfig, DeviceInfo, InputEvent};

type TestApp = App<MockDisplay, MockInput, MockClock, MockTransport>;

fn app_with(input: MockInput) -> TestApp {
    App::new(
        MockDisplay::new(),
        input,
        MockClock::new(),
        AppConfig {
            vid: 0x0482,
            type2: false,
        },
    )
}

fn device() -> DeviceInfo {
    DeviceInfo {
        vid: 0x0482,
        sn: Some("SKYKA077394".to_string()),
    }
}

/// Scripted phone for a complete `readuimlk` session (desktop test vector:
/// challenge `01 23 45 67 89 AB CD EF`, expected `0x53` reply
/// `53050602070601030d0b0c000d0b070f01`).
fn read_uimlk_transport() -> MockTransport {
    let challenge_frame: [u8; 17] = [
        0x52, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
        0x0e, 0x0f,
    ];
    let auth_frame: [u8; 17] = [
        0x53, 0x05, 0x06, 0x02, 0x07, 0x06, 0x01, 0x03, 0x0d, 0x0b, 0x0c, 0x00, 0x0d, 0x0b, 0x07,
        0x0f, 0x01,
    ];
    let state_frame = [0x74u8, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

    MockTransport::new()
        .on_write(&[0xD8], &[])
        .on_write(&[0x1C, 0x05], &[0x1C, 0x05])
        .on_write(&[0xE7], &[0xE7])
        .on_write(&[0x52], &challenge_frame)
        .on_write(&auth_frame, &[0x53, 0x01])
        .on_write(&[0x54], &state_frame)
        .on_write(&[0x56], &[0x56])
}

#[test]
fn read_uimlk_runs_end_to_end_and_renders_contract_lines() {
    let mut input = MockInput::new();
    input.push(InputEvent::EncoderCw);
    input.push(InputEvent::EncoderPress);

    let mut app = app_with(input);
    app.attach(read_uimlk_transport(), device());
    app.tick();

    let display = app.display();
    assert!(
        display.line(0).trim_start().starts_with("readuimlk"),
        "row0: {}",
        display.line(0)
    );
    assert!(
        display.line(1).contains("12345678"),
        "row1: {}",
        display.line(1)
    );
    assert!(
        display.line(2).contains("ctrl 0"),
        "row2: {}",
        display.line(2)
    );

    let transport = app.link_mut().expect("transport attached");
    assert!(
        transport.is_clean(),
        "violations: {:?}",
        transport.violations
    );
    assert_eq!(transport.remaining_steps(), 0, "script fully consumed");
}

#[test]
fn confirm_page_never_sends_a_frame() {
    let mut input = MockInput::new();
    for _ in 0..3 {
        input.push(InputEvent::EncoderCw); // Status -> Read -> Write -> Clear UIM lock
    }
    input.push(InputEvent::EncoderPress);

    let mut app = app_with(input);
    app.attach(MockTransport::new(), device());
    app.tick();

    let display = app.display();
    assert!(
        display.line(0).contains("CONFIRM"),
        "row0: {}",
        display.line(0)
    );
    assert!(
        display.line(3).contains("NOT UNDOABLE"),
        "row3: {}",
        display.line(3)
    );
    assert!(
        app.link_mut()
            .expect("transport attached")
            .writes
            .is_empty(),
        "confirm page must not talk to the phone"
    );
}

#[test]
fn short_press_then_long_press_confirms_a_destructive_operation() {
    let mut input = MockInput::new();
    for _ in 0..3 {
        input.push(InputEvent::EncoderCw);
    }
    input.push(InputEvent::EncoderPress); // confirm page
    input.push(InputEvent::EncoderPress); // ignored: no single-key submit
    input.push(InputEvent::EncoderLongPress); // confirmed submit

    // No transport attached: the submit path must surface the NO DEVICE page
    // instead of running anything.
    let mut app = app_with(input);
    app.tick();

    let display = app.display();
    assert!(
        display.line(0).contains("NO DEVICE"),
        "row0: {}",
        display.line(0)
    );
}

#[test]
fn submit_without_phone_shows_no_device_page() {
    let mut input = MockInput::new();
    input.push(InputEvent::EncoderPress); // Status is non-destructive -> submit

    let mut app = app_with(input);
    app.tick();

    assert!(app.display().line(0).contains("NO DEVICE"));
}

#[test]
fn detach_resets_ui_to_the_menu() {
    let mut app = app_with(MockInput::new());
    app.attach(MockTransport::new(), device());
    app.tick();

    // Navigate into the PIN/ICCID submenu before detaching.
    app.input_mut().push(InputEvent::EncoderCw);
    app.input_mut().push(InputEvent::EncoderCw);
    app.input_mut().push(InputEvent::EncoderCw);
    app.input_mut().push(InputEvent::EncoderCw);
    app.input_mut().push(InputEvent::EncoderPress);
    app.tick();
    assert!(app.display().line(0).contains("PIN / ICCID"));

    app.detach();
    app.tick();
    let display = app.display();
    assert!(
        display.line(0).contains("NO DEVICE"),
        "row0: {}",
        display.line(0)
    );
    assert!(
        display.line(1).contains("Status"),
        "row1: {}",
        display.line(1)
    );
}
