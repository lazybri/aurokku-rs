//! Locate and link the static libwdi produced by this branch's pipeline.
//!
//! Contract (see README.md):
//! - `LIBWDI_LIB_DIR` unset  -> only warn. This keeps `cargo check`,
//!   `cargo metadata` and default-feature builds working on machines that do
//!   not have the artifact (the crate is an *optional* dependency there).
//! - `LIBWDI_LIB_DIR` set but the directory has no `libwdi.lib` -> panic.
//!   An explicitly requested static libwdi that cannot be found must fail
//!   loudly instead of producing an executable that silently falls back to the
//!   external `wdi-simple.exe` path.
//! - Success -> `-L native` + `-l static=libwdi` + the 19 system import
//!   libraries libwdi needs.
//!
//! Note: objects from a native static library are merged into a Rust
//! `staticlib` (the product's `auroku_ffi.lib`), so consumers must NOT link
//! `libwdi.lib` again; they only have to make sure `LIBWDI_LIB_DIR` is set
//! while cargo builds the staticlib. The system DLLs below are normally also
//! resolved from the `.drectve` records of the libwdi objects; listing them
//! keeps MinGW builds and link-order-sensitive setups working.

fn main() {
    println!("cargo:rerun-if-env-changed=LIBWDI_LIB_DIR");

    let Some(dir) = std::env::var_os("LIBWDI_LIB_DIR").map(std::path::PathBuf::from) else {
        println!(
            "cargo:warning=LIBWDI_LIB_DIR is not set; the static libwdi will be missing at \
             link time. Download the pinned libwdi-static artifact (see README.md) and \
             export LIBWDI_LIB_DIR=<artifact>/lib."
        );
        return;
    };

    let library = dir.join("libwdi.lib");
    if !library.is_file() {
        panic!(
            "LIBWDI_LIB_DIR={} does not contain libwdi.lib; download the pinned \
             libwdi-1.5.1-x64 artifact and point LIBWDI_LIB_DIR at its lib/ directory",
            dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=libwdi");

    // System import libraries used by libwdi. MSVC also honours the directives
    // recorded in the libwdi objects, but listing them keeps MinGW builds and
    // link-order-sensitive setups working.
    for lib in [
        "setupapi", "ole32", "ntdll", "advapi32", "user32", "gdi32", "comctl32", "crypt32",
        "wintrust", "version", "shlwapi", "cfgmgr32", "msi", "shell32", "oleaut32", "uuid",
        "comdlg32", "winspool", "userenv",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
