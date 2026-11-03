//! Link-time smoke test for the static libwdi.
//!
//! This test never installs a driver. It exists so that
//! `cargo test -p libwdi-sys --no-run` has a test harness
//! that references \`install_winusb\`: the linker therefore has to resolve
//! \`libwdi.lib\` and the 19 system import libraries, which turns the CI
//! "crate" job into a real link smoke test for the artifact.
#![cfg(windows)]

#[test]
fn libwdi_is_linkable() {
    // Taking the function address forces the object file that references the
    // wdi_* symbols into the test binary.
    let install: fn(u16, u16) -> Result<(), libwdi_sys::WdiError> = libwdi_sys::install_winusb;
    assert!(std::mem::size_of_val(&install) > 0);

    // Public constants mirrored from libwdi.h.
    assert_eq!(libwdi_sys::WDI_SUCCESS, 0);
    assert_eq!(libwdi_sys::WDI_ERROR_IO, -1);
    assert_eq!(libwdi_sys::WDI_ERROR_INVALID_PARAM, -2);
    assert_eq!(libwdi_sys::WDI_ERROR_NEEDS_ADMIN, -15);
}
