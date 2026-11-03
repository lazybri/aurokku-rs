//! Locate the static libwdi library built by CI.
//!
//! GitHub Actions builds libwdi (static, /MT, WinUSB payload only) and exports
//! LIBWDI_LIB_DIR. Local builds only need this when the static-libwdi feature is
//! explicitly enabled.

fn main() {
    println!("cargo:rerun-if-env-changed=LIBWDI_LIB_DIR");

    let Ok(dir) = std::env::var("LIBWDI_LIB_DIR") else {
        println!(
            "cargo:warning=LIBWDI_LIB_DIR is not set; building with the static-libwdi \
             feature needs a libwdi.lib from CI (see docs/windows-driver.md)"
        );
        return;
    };

    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=libwdi");

    // System import libraries used by libwdi. MSVC also honours the directives
    // recorded in the libwdi objects, but listing them keeps MinGW builds and
    // link-order-sensitive setups working.
    for lib in [
        "setupapi", "ole32", "ntdll", "advapi32", "user32", "gdi32", "comctl32", "crypt32",
        "wintrust", "version", "shlwapi", "cfgmgr32", "msi",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
