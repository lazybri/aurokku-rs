//! Host-side parity checks for the scenarios executed by the `rp2350-emu`
//! test firmware (`firmware/rp2354-emu-fw`).
//!
//! The emulator runs the very same scenarios on the simulated RP2354; keeping a
//! host version makes UI/protocol expectation bugs easy to diagnose (the host
//! can print the exact mocked writes/violations, the emulator only a hash).

use auroku_core::encode_nibbles;
use auroku_core::mock::MockTransport;
use auroku_core::type2auth::{compute_type2auth_request, parse_sn_to_type2auth};
use auroku_firmware::mock::{MockClock, MockDisplay, MockInput};
use auroku_firmware::{App, AppConfig, DeviceInfo, InputEvent};

fn read_uimlk_transport() -> MockTransport {
    let challenge: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

    let mut challenge_frame = [0u8; 17];
    challenge_frame[0] = 0x52;
    challenge_frame[1..].copy_from_slice(&encode_nibbles(&challenge));

    let (material, _sn) = parse_sn_to_type2auth("SKYKA077394").expect("SN material");
    let auth_frame = compute_type2auth_request("uimlk", &challenge, &material).expect("auth frame");

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
fn emu_scenario_read_uimlk_parity() {
    let mut input = MockInput::new();
    input.push(InputEvent::EncoderCw); // Status -> Read UIM lock
    input.push(InputEvent::EncoderPress); // submit

    let mut app = App::new(
        MockDisplay::new(),
        input,
        MockClock::new(),
        AppConfig {
            vid: 0x0482,
            type2: false,
        },
    );
    app.attach(
        read_uimlk_transport(),
        DeviceInfo {
            vid: 0x0482,
            sn: Some("SKYKA077394".to_string()),
        },
    );
    app.tick();

    println!("--- screen ---\n{}", app.display().screen());
    let transport = app.link_mut().expect("attached");
    println!("--- writes ---\n{:#02x?}", transport.writes);
    println!("--- violations ---\n{:?}", transport.violations);
    println!("remaining steps: {}", transport.remaining_steps());

    assert!(
        transport.violations.is_empty(),
        "mock violations: {:?}",
        transport.violations
    );
    assert_eq!(transport.remaining_steps(), 0);
}

#[test]
fn emu_scenario_confirm_screen_parity() {
    let mut input = MockInput::new();
    for _ in 0..3 {
        input.push(InputEvent::EncoderCw);
    }
    input.push(InputEvent::EncoderPress);

    let mut app = App::new(
        MockDisplay::new(),
        input,
        MockClock::new(),
        AppConfig {
            vid: 0x0482,
            type2: false,
        },
    );
    app.attach(
        MockTransport::new(),
        DeviceInfo {
            vid: 0x0482,
            sn: None,
        },
    );
    app.tick();

    println!("--- confirm screen ---\n{}", app.display().screen());
    let transport = app.link_mut().expect("attached");
    assert!(transport.writes.is_empty(), "no frames from confirm page");
}
