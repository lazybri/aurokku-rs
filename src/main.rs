#![forbid(unsafe_code)]

//! auRokku CLI entry point. All protocol and reporting logic lives in the
//! `auroku` library, which is also consumed by the Qt front-end through the
//! `auroku-ffi` C ABI crate.

fn main() {
    auroku::cli_main();
}
