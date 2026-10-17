#![forbid(unsafe_code)]

//! auRokku CLI entry point. All protocol and reporting logic lives in the
//! `auroku` library so the egui front-end (`auroku-gui`) can share it.

fn main() {
    auroku::cli_main();
}
