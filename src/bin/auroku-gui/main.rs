#![forbid(unsafe_code)]

//! Material 3 desktop front-end for auRokku.
//!
//! Build / run:
//!   cargo run --release --features gui --bin auroku-gui
//!
//! Platform notes:
//! - Linux: eframe enables both Wayland and X11; the Wayland app id is `auroku`.
//! - Windows: native Win32 window + OpenGL (glow).
//! - macOS: native Cocoa window + OpenGL (glow).

mod app;
mod fonts;
mod i18n;
mod permissions;
mod theme;
mod widgets;
mod worker;

use eframe::egui;
use std::ffi::OsStr;

fn main() -> eframe::Result {
    i18n::set(i18n::Language::from_env());

    if let Some(code) = permissions::run_privileged_helper() {
        std::process::exit(code);
    }

    if handle_cli_args() {
        return Ok(());
    }

    if permissions::refuse_root_execution() {
        std::process::exit(1);
    }

    let options = eframe::NativeOptions {
        // `with_app_id` is what Wayland compositors use for window matching and
        // the task switcher. It is ignored on Windows and macOS.
        viewport: egui::ViewportBuilder::default()
            .with_app_id("auroku")
            .with_title("auRokku")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([980.0, 660.0]),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "auRokku",
        options,
        Box::new(|cc| Ok(Box::new(app::AurokuApp::new(cc)))),
    )
}

/// Intercept the CLI flags the GUI understands.
/// Returns `true` when the process should exit without opening a window.
fn handle_cli_args() -> bool {
    for arg in std::env::args_os().skip(1) {
        let arg = arg.as_os_str();
        if arg == OsStr::new("--help") || arg == OsStr::new("-h") {
            println!(
                "auRokku GUI — SERI / Type2Auth device tool\n\
                 \n\
                 Usage: auroku-gui\n\
                 \n\
                 Options:\n\
                   -h, --help     Show this help\n\
                   -V, --version  Show version\n\
                 \n\
                 The interface language follows LANG / LC_ALL (zh* = Chinese, otherwise\n\
                 English) and can be switched in the top bar.\n\
                 Linux: run as a normal user. Select a USB device to request\n\
                 permission for that device only. The GUI refuses to run as root."
            );
            return true;
        }
        if arg == OsStr::new("--version") || arg == OsStr::new("-V") {
            println!("auroku-gui {}", env!("CARGO_PKG_VERSION"));
            return true;
        }
    }
    false
}
