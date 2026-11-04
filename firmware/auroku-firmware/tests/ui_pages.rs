//! Typed result / error page tests for the OLED UI (docs/plan/09 §5.4–5.5).

use auroku_core::report::{
    BatchError, BatchReport, BatchResult, ExitBaseReport, ExitOutcome, ExitUimlkReport, FieldValue,
    IccidReport, MaintenanceReport, MutationReport, PinReport, ProbeReport, ReadUimlkReport,
    Report, ResetPinReport, StatusReport, UimLockState, UimLockSummary,
};
use auroku_core::{AuRokkuError, BatchProfile, ProbeKind, Task, TaskOutcome};
use auroku_firmware::mock::MockDisplay;
use auroku_firmware::protocol::ProtoOutcome;
use auroku_firmware::ui::{UiAction, UiState};
use auroku_firmware::{DeviceInfo, InputEvent};

fn render(ui: &UiState) -> MockDisplay {
    let mut display = MockDisplay::new();
    ui.render(&mut display);
    display
}

fn report(task: Task, report: Report) -> ProtoOutcome {
    ProtoOutcome {
        task,
        result: Ok(TaskOutcome {
            report,
            exit: None,
            async_events: Vec::new(),
        }),
    }
}

fn error(task: Task, error: AuRokkuError) -> ProtoOutcome {
    ProtoOutcome {
        task,
        result: Err(error),
    }
}

fn state(control: u8, value: &str) -> UimLockState {
    UimLockState {
        control,
        lock_value: value.to_string(),
        binding_cleared: value == "ffffffff",
        raw_hex: "74 00 01 02 03 04 05 06 07 08".to_string(),
    }
}

#[test]
fn read_uimlk_page_matches_emulator_contract() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::ReadUimlk,
        Report::ReadUimlk(ReadUimlkReport {
            state: state(0, "12345678"),
        }),
    ));
    let display = render(&ui);
    assert!(
        display.line(0).starts_with("readuimlk"),
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
    assert!(
        display.line(3).contains("binding"),
        "row3: {}",
        display.line(3)
    );
}

#[test]
fn read_uimlk_page_pages_to_the_exit_result() {
    let mut ui = UiState::new();
    ui.on_outcome(&ProtoOutcome {
        task: Task::ReadUimlk,
        result: Ok(TaskOutcome {
            report: Report::ReadUimlk(ReadUimlkReport {
                state: state(1, "80e48ed2"),
            }),
            exit: Some(ExitOutcome {
                acknowledged: true,
                reply: Some("56".to_string()),
                error: None,
                may_reboot: true,
            }),
            async_events: Vec::new(),
        }),
    });
    let display = render(&ui);
    assert!(
        display.line(5).contains("1/2"),
        "footer: {}",
        display.line(5)
    );
    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    let display = render(&ui);
    let page = display.screen();
    assert!(page.contains("exit ack OK"), "second page: {page}");
    assert!(page.contains("may reboot"), "second page: {page}");
}

#[test]
fn mutation_page_shows_before_and_after_on_one_screen() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Writelk {
            value: "80e48ed2".to_string(),
            control: 1,
        },
        Report::Mutation(MutationReport {
            operation: "writelk",
            before: state(0, "ffffffff"),
            reply: Some("72 01".to_string()),
            command_accepted: true,
            command_error: None,
            after: Some(state(1, "80e48ed2")),
            verified: true,
            readback_error: None,
            success: true,
            partial_write_possible: false,
            persistence_verified: false,
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("writelk"));
    assert!(display.line(1).contains("before FFFF FFFF"));
    assert!(display.line(2).contains("after  80E4 8ED2"));
    assert!(display.line(3).contains("verified OK"));
}

#[test]
fn failed_mutation_page_flags_partial_write() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Clearlk,
        Report::Mutation(MutationReport {
            operation: "clearlk",
            before: state(1, "80e48ed2"),
            reply: None,
            command_accepted: false,
            command_error: Some("USB transfer timed out".to_string()),
            after: None,
            verified: false,
            readback_error: None,
            success: false,
            partial_write_possible: true,
            persistence_verified: false,
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("clearlk"));
    assert!(display.line(3).contains("VERIFY NG"));
    let page = display.screen();
    assert!(page.contains("PARTIAL WRITE POSS."), "page: {page}");
}

#[test]
fn iccid_page_pages_long_hex_and_decoded_value() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::ReadIccid,
        Report::Iccid(IccidReport {
            raw_hex: "59 98 81 10 12 34 56 78 90 12 34 56 78 90 12 34 56 78 90 12 34".to_string(),
            decoded_hex: "98 81 10 12 34 56 78 90 12 34".to_string(),
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("readiccid"));
    assert!(display.line(1).contains("raw:"));
    assert!(
        display.line(5).contains("1/2"),
        "footer: {}",
        display.line(5)
    );

    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    let display = render(&ui);
    assert!(
        display.line(1).contains("decoded:"),
        "row1: {}",
        display.line(1)
    );
    assert!(display.line(5).contains("2/2"));

    // BACK returns to the menu.
    assert_eq!(ui.handle(InputEvent::BackPress, 0), UiAction::None);
    assert!(render(&ui).line(0).contains("auRokku"));
}

#[test]
fn pin_and_reset_pin_pages_show_values() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Readpin,
        Report::Pin(PinReport {
            lockcode: "1234".to_string(),
            raw_hex: "dc 00 91 92 93 94 aa".to_string(),
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("readpin"));
    assert!(display.line(1).contains("PIN 1234"));

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Resetpin,
        Report::ResetPin(ResetPinReport {
            reply: "dc01".to_string(),
            lockcode: "1234".to_string(),
            verified: true,
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("resetpin"));
    assert!(display.line(1).contains("PIN 1234"));
    assert!(display.line(2).contains("verified OK"));
}

#[test]
fn maintenance_batch_probe_and_exit_pages_render() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::MaintenanceBit { control: 1 },
        Report::Maintenance(MaintenanceReport {
            maintenance_bit: 1,
            reply: "72 01".to_string(),
            command_accepted: true,
            success: true,
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("maintenance_bit"));
    assert!(display.line(1).contains("bit 1"));

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Batch {
            profile: BatchProfile::Basic,
        },
        Report::Batch(BatchReport {
            profile: BatchProfile::Basic,
            success: true,
            results: vec![
                BatchResult::Version {
                    raw_hex: "1b 01 02 03 04".to_string(),
                },
                BatchResult::Lockcode {
                    raw_hex: "dc 00 91 92 93 94 aa".to_string(),
                    decoded: Some("1234".to_string()),
                },
            ],
            error: None,
            automatic_exit: false,
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("batch"));
    assert!(display.line(1).contains("profile basic"));
    let page = display.screen();
    assert!(page.contains("[OK] v "), "page: {page}");
    assert!(page.contains("[OK] pin 1234"), "page: {page}");

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Batch {
            profile: BatchProfile::Uimlk,
        },
        Report::Batch(BatchReport {
            profile: BatchProfile::Uimlk,
            success: false,
            results: Vec::new(),
            error: Some(BatchError {
                kind: "ValueError",
                message: "Unsupported batch profile: 'x'".to_string(),
            }),
            automatic_exit: false,
        }),
    ));
    let page = render(&ui).screen();
    assert!(page.contains("[NG] ValueError:"), "page: {page}");

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Probe {
            kind: ProbeKind::Seri,
            timeout_ms: 1000,
        },
        Report::Probe(ProbeReport {
            probe: ProbeKind::Seri,
            request_hex: Some("1c 05".to_string()),
            response_hex: Some("1c 05".to_string()),
            responsive: true,
            matched: Some(true),
            cleanup_hex: Some("d8".to_string()),
            response_length: None,
            success: true,
        }),
    ));
    let page = render(&ui).screen();
    assert!(page.contains("kind seri"), "page: {page}");
    assert!(page.contains("matched OK"), "page: {page}");
    assert!(page.contains("req 1c 05"), "page: {page}");
    // Cleanup/response details continue on the second page.
    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    let page = render(&ui).screen();
    assert!(page.contains("cleanup d8"), "page: {page}");

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Exituimlk,
        Report::ExitUimlk(ExitUimlkReport {
            reply: "56".to_string(),
            success: true,
            method: "handshake_and_exit",
        }),
    ));
    let page = render(&ui).screen();
    assert!(page.contains("exituimlk"), "page: {page}");
    assert!(page.contains("phone may reboot"), "page: {page}");

    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Exit,
        Report::ExitBase(ExitBaseReport {
            success: true,
            method: "0xd8",
        }),
    ));
    let page = render(&ui).screen();
    assert!(page.contains("method 0xd8"), "page: {page}");
}

#[test]
fn status_page_summarises_fields() {
    let mut ui = UiState::new();
    ui.on_outcome(&report(
        Task::Status,
        Report::Status(StatusReport {
            sn: Some("SKYKA077394".to_string()),
            version: Some(FieldValue::Value("1b 01 02 03 04".to_string())),
            lockcode: Some(FieldValue::Value("1234".to_string())),
            uim_lock: Some(UimLockSummary {
                control: 1,
                lock_value: "80e48ed2".to_string(),
                unlocked: false,
            }),
            iccid_record: Some(FieldValue::Value("98 81 10 12 34".to_string())),
        }),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("status"));
    let page = display.screen();
    assert!(page.contains("sn SKYKA077394"), "page: {page}");
    assert!(page.contains("PIN 1234"), "page: {page}");
    assert!(page.contains("UIM 80E4 8ED2 ctl 1"), "page: {page}");
}

#[test]
fn error_pages_map_every_error_kind() {
    for (err, kind) in [
        (AuRokkuError::Value("bad value".to_string()), "ValueError"),
        (
            AuRokkuError::Runtime("device rejected".to_string()),
            "RuntimeError",
        ),
        (
            AuRokkuError::Timeout("USB transfer timed out".to_string()),
            "TimeoutError",
        ),
        (
            AuRokkuError::Driver("WinUSB missing".to_string()),
            "DriverError",
        ),
    ] {
        let mut ui = UiState::new();
        ui.on_outcome(&error(Task::Status, err));
        let display = render(&ui);
        assert!(
            display.line(0).contains(kind),
            "row0 {:?} should contain {kind}",
            display.line(0)
        );
    }
}

#[test]
fn timeout_error_page_offers_retry_of_the_last_task() {
    let mut ui = UiState::new();
    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::Status)
    );
    ui.on_outcome(&error(
        Task::Status,
        AuRokkuError::Timeout("USB transfer timed out".to_string()),
    ));
    let display = render(&ui);
    assert!(display.line(0).contains("ERROR TimeoutError"));
    assert!(
        display.line(5).contains("OK=retry"),
        "footer: {}",
        display.line(5)
    );
    assert_eq!(
        ui.handle(InputEvent::EncoderPress, 0),
        UiAction::Submit(Task::Status)
    );
}

#[test]
fn no_device_page_and_back_to_menu() {
    let mut ui = UiState::new();
    ui.on_no_device();
    let display = render(&ui);
    assert!(display.line(0).contains("NO DEVICE"));
    assert!(display.line(1).contains("attach phone"));
    assert_eq!(ui.handle(InputEvent::BackPress, 0), UiAction::None);
    assert!(render(&ui).line(0).contains("auRokku"));
}

#[test]
fn attach_shows_sn_and_detach_resets_the_screen() {
    let mut ui = UiState::new();
    ui.on_attach(DeviceInfo {
        vid: 0x0482,
        sn: Some("SKYKA077394".to_string()),
    });
    assert!(render(&ui).line(0).contains("SKYKA077394"));

    ui.on_outcome(&report(
        Task::Status,
        Report::ExitBase(ExitBaseReport {
            success: true,
            method: "0xd8",
        }),
    ));
    assert!(render(&ui).line(0).contains("exit"));

    ui.on_detach();
    let display = render(&ui);
    assert!(display.line(0).contains("NO DEVICE"));
    assert!(display.line(1).contains("Status"));
    // Detached UI must still accept navigation (no stale result page).
    assert_eq!(ui.handle(InputEvent::EncoderCw, 0), UiAction::None);
    assert!(render(&ui).line(2).contains("Read UIM lock"));
}
