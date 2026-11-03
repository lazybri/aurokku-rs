//! Diagnostics for the GUI process.
//!
//! Release builds have no console, so every line is mirrored to gui.log in the
//! auRokku state directory (next to driver.log).

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Write one diagnostic line to stderr (when attached) and to gui.log.
pub fn log(message: &str) {
    eprintln!("[auroku-gui] {message}");

    static FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
    let handle = FILE.get_or_init(|| {
        let path = auroku::driver::state_dir().join("gui.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Mutex::new(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .ok(),
        )
    });

    if let Ok(mut guard) = handle.lock() {
        if let Some(file) = guard.as_mut() {
            let _ = writeln!(file, "{message}");
        }
    }
}
