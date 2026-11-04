//! `rp2350-emu` mock tests for the RP2354 firmware.
//!
//! These are the first tier of the mock-test chain required by docs/plan/13
//! §1.6: the firmware runs as a real Cortex-M33 image inside `rp2350-emu`, with
//! a scripted phone transport and mock OLED/encoder. Only after every scenario
//! here passes should Kablix-based mock tests be added.

use auroku_emu_tests::{
    firmware_elf_from_env, run_scenario, EmuOutcome, SCENARIO_ALLOC, SCENARIO_CONFIRM_SCREEN,
    SCENARIO_NO_DEVICE, SCENARIO_READ_UIMLK,
};

fn run_and_check(scenario: u32) -> EmuOutcome {
    let Some(path) = firmware_elf_from_env() else {
        return EmuOutcome {
            status: 1,
            check: 0,
            detail: 0,
            hash: 0,
            debug0: 0,
            debug1: 0,
            debug2: 0,
            debug3: 0,
            cycles: 0,
        };
    };
    let outcome = run_scenario(&path, scenario);
    assert!(
        outcome.passed(),
        "rp2350-emu scenario {scenario} FAILED: status={} check=0x{:08x} detail=0x{:08x} \
         cycles={} (inspect with `rp2350-emu-tui --elf {}`)",
        outcome.status,
        outcome.check,
        outcome.detail,
        outcome.cycles,
        path.display()
    );
    eprintln!(
        "rp2350-emu scenario {scenario}: PASS (screen hash 0x{:08x}, {} cycles)",
        outcome.hash, outcome.cycles
    );
    outcome
}

#[test]
fn allocator_and_vec_work_on_emulated_rp2354() {
    let outcome = run_and_check(SCENARIO_ALLOC);
    assert_ne!(outcome.hash, 0, "alloc scenario should report a non-zero hash");
}

#[test]
fn read_uimlk_session_runs_end_to_end_on_emulated_rp2354() {
    run_and_check(SCENARIO_READ_UIMLK);
}

#[test]
fn destructive_operation_requires_confirm_page_on_emulated_rp2354() {
    run_and_check(SCENARIO_CONFIRM_SCREEN);
}

#[test]
fn submit_without_phone_shows_no_device_on_emulated_rp2354() {
    run_and_check(SCENARIO_NO_DEVICE);
}
