//! Pull in the custom RP2354 SRAM linker script for this crate's binary.

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-search=native={manifest_dir}");
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=build.rs");
}
