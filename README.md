**English** | [简体中文](README.zh-CN.md)

# auRokku (Rust)

A Rust rewrite of `auRokku.py`; protocol behaviour, the Type2Auth authentication
algorithm, JSON structure and CLI surface are kept compatible. Thanks to Yuu for
the original Python source.

## Features

- UIM lock read / write / clear (0x54 / 0x72 / 0x57)
- Phone Lock Code (PIN, NV 82) read / reset (0xDC 0x00 / 0xDC 0x01)
- NV 10481 maintenance mode bit (0xE1 + auth 4000)
- ICCID record read (0x59)
- Single-session status / batch queries
- USB layout enumeration, SERI echo and SCDP active probe (`--op probe`)
- Type2Auth 52/53 authentication (`--type2`)
- Optional JSON output and verbose USB frame logs
- egui + eframe cross-platform GUI (Material 3, Chinese/English UI, Wayland / Windows / macOS)

## GUI front-end (egui / Material 3)

`auroku-gui` is a desktop front-end sharing the same `auroku` backend library as
the CLI. The UI uses custom-painted Material 3 components (navigation rail,
cards, filled / tonal / outlined buttons, switches, segmented buttons, status
chips, snackbars, confirmation dialogs), with light and dark colour schemes and
a Chinese/English interface.

- **Language**: the selector in the top bar switches between 简体中文 and English.
  On first start the language follows `LANG` / `LC_MESSAGES` / `LC_ALL`
  (`zh*` → Chinese, other set locales → English; unset → Chinese). If no CJK
  font is available, the UI falls back to English automatically.

All USB operations run on a background thread, so the UI never blocks on libusb
transfers or timeouts.

```bash
# Development
cargo run --features gui --bin auroku-gui

# Release (same command on Linux / Windows / macOS)
cargo build --release --features gui --bin auroku-gui

# Linux: disable pkg-config to avoid linking the host libudev into the GUI
PKG_CONFIG=/bin/false cargo build --release --features gui --bin auroku-gui
```

Output: `target/<target>/release/auroku-gui` (`auroku-gui.exe` on Windows).

> The Linux GUI is **dynamically linked**: it depends at least on `libc`, `libm`
> and `libgcc_s`; Wayland/X11 and OpenGL are loaded at runtime by
> winit/glutin. It cannot be a fully static, runnable binary. See
> “Static linking status” below.

Platform notes:

- **Linux**: eframe enables both Wayland and X11; the Wayland `app_id` is
  `auroku`, and X11/Wayland client libraries are loaded via `dlopen` at runtime.
  `build-linux.sh` builds the static musl CLI and the host-glibc GUI in one
  run (`dist/auroku-linux-x86_64` and `dist/auroku-gui-linux-x86_64`). Static
  musl does not support `dlopen`, so a static GUI cannot start;
  `PKG_CONFIG=/bin/false` avoids the host `libudev` dependency, and the
  remaining dynamic dependencies are `libc` / `libm` / `libgcc_s`.
- **Windows**: native Win32 window + OpenGL (glow), no extra runtime required.
- **macOS**: native Cocoa window + OpenGL (glow). For a universal binary, build
  `aarch64-apple-darwin` and `x86_64-apple-darwin` and combine them with `lipo`.
- **Fonts**: a system CJK font is located at start-up (Noto Sans CJK /
  Microsoft YaHei / PingFang, etc.). Override it with
  `AUROKU_GUI_FONT=/path/to/font.ttf`.
- **Permissions**: USB access on Linux usually requires root or a matching udev
  rule; on Windows a libusb driver may be required (same as the CLI).

### Linux polkit elevation

On Linux the GUI requests root through `pkexec` (polkit) at start-up by default.
After successful authentication it re-executes itself as root so it can access
USB devices directly:

- Skip elevation with `--no-polkit` or `AUROKU_GUI_NO_POLKIT=1`.
- If authentication is cancelled, polkit is unavailable or `pkexec` is missing,
  the app continues as a normal user and shows a permission warning on the
  overview page; USB operations may fail due to insufficient permissions.
- Only display / session / locale variables are forwarded (Wayland, X11, DBus,
  locale); sensitive variables such as `LD_*`, `PATH` and `HOME` are not.
  World-writable executables are refused.
- Some Wayland compositors restrict root clients. If root mode cannot connect to
  the display, try `WINIT_UNIX_BACKEND=x11` (requires XWayland), or configure a
  udev rule and run unprivileged.
- Security note: running the whole process as root increases the attack surface.
  If you do not need automatic elevation, skip polkit and configure a udev rule
  for the device.

The GUI’s Overview / UIM Lock / PIN & ICCID / Probe / Batch / Session / Console
pages cover every CLI `--op` operation. The console page shows USB TX/RX logs and
the full JSON result.

## Building

Requires a Rust toolchain (<https://rustup.rs>). `rusb` uses vendored libusb, so
a C compiler is also required (`gcc`/`clang` on Linux/macOS, MSVC Build Tools or
MinGW-w64 on Windows).

Each platform has a local build script; artifacts go to `dist/`. The Linux,
Windows and macOS native scripts build **both the CLI and the GUI**. “Static”
here refers to Rust/C
runtime dependencies; whether system libraries and the graphics stack can be
linked statically is determined by the OS:

| Platform | Command | Artifacts |
| --- | --- | --- |
| Linux (static CLI + host glibc GUI) | `chmod +x build-linux.sh && ./build-linux.sh` | `dist/auroku-linux-x86_64` (static ELF), `dist/auroku-gui-linux-x86_64` (glibc) |
| Windows (static CRT) | `.\build-windows.ps1` | `dist\auroku.exe`, `dist\auroku-gui.exe`, `dist\auroku-windows-<arch>.zip` |
| macOS (universal) | `chmod +x build-macos.sh && ./build-macos.sh` | `dist/auroku`, `dist/auroku-gui` (arm64 + x86_64) |
| macOS (native arch) | `./build-macos.sh --native` | `dist/auroku`, `dist/auroku-gui` |

Static linking status:

| Artifact | Static status |
| --- | --- |
| Linux CLI (`auroku`) | **Fully static**: musl + static CRT, no `DT_NEEDED`, no `PT_INTERP` |
| Windows CLI (`auroku.exe`) | Static CRT (`crt-static`); system DLLs such as `kernel32`/`user32` are provided by Windows and cannot be static |
| macOS CLI/GUI | libSystem and system frameworks must be dynamic (macOS limitation); Rust dependencies and libusb are static |
| Linux GUI (`auroku-gui`) | Dynamically linked against glibc and system graphics libraries; **cannot** be fully static |
| Windows GUI (`auroku-gui.exe`) | Static CRT + system DLLs / OpenGL |

The Linux GUI relies on winit/glutin loading Wayland/X11/OpenGL via `dlopen` at
runtime. Static musl binaries do not support `dlopen` and fail with
`Dynamic loading not supported` / `NoWaylandLib`. Build the GUI against the host
glibc target; do not use the static musl target.

### Static linking verification

Verify the static Linux CLI with the following commands (tested on x86_64):

```bash
file dist/auroku-linux-x86_64
# ELF 64-bit LSB executable ... statically linked
readelf -d dist/auroku-linux-x86_64 | grep NEEDED   # no output
readelf -l dist/auroku-linux-x86_64 | grep INTERP   # no output
ldd dist/auroku-linux-x86_64                        # not a dynamic executable
```

`build-linux.sh` performs these checks after the build and exits on failure.

Linux builds require `musl-gcc` and kernel UAPI headers:

```bash
# Debian/Ubuntu
sudo apt-get install musl-tools linux-libc-dev
# Fedora
sudo dnf install musl-gcc kernel-headers
# Arch
sudo pacman -S musl linux-api-headers
```

You can also use Cargo directly. `.cargo/config.toml` enables static linking for
musl and Windows targets; on Linux the script also prepares the kernel UAPI
headers. When building directly, disable pkg-config so the host glibc `libudev`
is not linked into the static binary:

```bash
PKG_CONFIG=/bin/false \
CC_x86_64_unknown_linux_musl=musl-gcc \
AR_x86_64_unknown_linux_musl=ar \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
C_INCLUDE_PATH=/usr/include:/usr/include/x86_64-linux-gnu \
  cargo build --release --target x86_64-unknown-linux-musl --bin auroku
```

Output: `target/x86_64-unknown-linux-musl/release/auroku` (static ELF). Using
`build-linux.sh` is recommended: it prepares the UAPI headers and verifies static
linkage automatically.

### Cross-compiling Linux (x86_64 static musl)

On Windows you can cross-compile with Zig, without WSL / Docker:

```powershell
# One-time setup: install Zig, extract it to %USERPROFILE%\tools\zig-<version>,
# or set the ZIG environment variable / pass -ZigPath path\to\zig.exe
.\windows-cross-build-linux.ps1
```

Output: `target\x86_64-unknown-linux-musl\release\auroku` (x86_64 static ELF, no
runtime dependencies; USB access requires root or a matching udev rule).

This cross script intentionally ships only the static CLI: a static musl GUI
cannot `dlopen` Wayland/X11/OpenGL, and cross-linking a glibc GUI would need a
full Linux graphics sysroot. Build the GUI on Linux with `./build-linux.sh` (or
download the Linux archive from the release artifacts).

### Building the macOS universal binary (arm64 + x86_64)

Run on macOS (with Xcode Command Line Tools installed):

```bash
chmod +x build-macos.sh
./build-macos.sh
```

The script builds both binaries for `aarch64-apple-darwin` and
`x86_64-apple-darwin`, combines each pair with `lipo`, and applies an ad-hoc
signature.

Output: `dist/auroku` and `dist/auroku-gui` (Apple Silicon + Intel Mac).

To build only the current machine architecture, use `./build-macos.sh --native`
(faster).

> macOS does not support fully static executables: libSystem and system
> frameworks must be linked dynamically. The script already links all Rust
> dependencies and vendored libusb statically; no further static option is
> available.

## GitHub Actions

The workflow in `.github/workflows/build.yml` runs **tests → three-platform build
→ release** in one go:

- **Pull request**: `cargo fmt --check`, `cargo clippy -D warnings` and
  `cargo test`.
- **Manual dispatch**: enter a version (e.g. `v0.1.0`; a missing `v` is added
  automatically), optionally prerelease / draft. The workflow runs tests, builds
  Windows, Linux (static musl) and the macOS universal binary in parallel, then
  creates a GitHub Release and uploads all artifacts.
- **Pushing a `v*` tag**: the same test, build and release flow.

Artifacts (each package contains both `auroku` and `auroku-gui`):

- `auroku-windows-x86_64.zip`
- `auroku-linux-x86_64.tar.gz` (static musl CLI + host-glibc GUI)
- `auroku-macos-universal.zip` (arm64 + x86_64)

The Linux GUI is built on `ubuntu-22.04`, so it requires glibc 2.35 or newer;
the CLI in the same package remains fully static.

> Windows/Linux hosts cannot cross-compile macOS binaries directly: the vendored
> libusb needs IOKit/CoreFoundation headers and link stubs from the Apple SDK,
> which cannot be redistributed. The macOS build therefore runs on the Actions
> `macos-14` runner.

Protocol-only unit tests (nibble codec, SN parsing, key derivation, auth packet
vectors, etc.):

```bash
cargo test
```

## Usage examples

`--op`, `--vid` and `--pid` are required:

```bash
# Enumerate USB configurations / interfaces / bulk endpoint pairs
auroku --op probe --vid 0482 --pid 0a5c

# Active SERI echo / SCDP probe
auroku --op probe --vid 0482 --pid 0a5c --probe-kind seri
auroku --op probe --vid 0482 --pid 0a5c --probe-kind scdp

# Full status
auroku --op status --vid 0482 --pid 0a5c

# UIM lock
auroku --op readuimlk --vid 0482 --pid 0a5c
auroku --op clearlk --vid 0482 --pid 0a5c
auroku --op writelk --vid 0482 --pid 0a5c --val 80e48ed2 --control 1

# Phone Lock Code
auroku --op readpin --vid 0482 --pid 0a5c
auroku --op resetpin --vid 0482 --pid 0a5c

# Maintenance bit
auroku --op maintenance_bit --vid 0482 --pid 0a5c --control 1

# Batch queries / exit
auroku --op batch --vid 0482 --pid 0a5c --profile uimlk
auroku --op exit --vid 0482 --pid 0a5c
auroku --op exituimlk --vid 0482 --pid 0a5c

# JSON / verbose log / explicit SN / Type2Auth
auroku --op status --vid 0482 --pid 0a5c --json
auroku --op status --vid 0482 --pid 0a5c --verbose
auroku --op status --vid 0482 --pid 0a5c --sn SKYKA077394
auroku --op status --vid 0482 --pid 0a5c --sn 947307414B594B53 --type2
```

## Relationship to the Python version

| Python | Rust |
| --- | --- |
| `encode_nibbles` / `decode_nibbles` | Same-named functions |
| `decode_lockcode_digits` | Same-named function |
| `parse_sn_to_type2auth` | Same-named function (returns `(Vec<u8>, String)`) |
| `parse_usb_endpoint` | Same-named function (used as a clap `value_parser`) |
| `bitrev8` / `derive_type2auth_key` | Same-named functions |
| `compute_type2auth_request` | Same-named function |
| `AuRokkuDevice` | Same-named `struct` (`rusb` transport layer) |
| `open_device` / `connect` / `close` | Same-named methods |
| `inspect_usb` / `probe` / `_read_probe_response` | Same-named methods |
| `authenticate_step` / `enter_base` / `enter_uimlk` | Same-named methods |
| `print_human_report` | Same-named function |

Implementation details:

- The Rust version selects Bulk/Interrupt transfer types from endpoint
  descriptors, matching pyusb `read`/`write` auto-detection.
- When `--sn` is 16 hex digits, it is used verbatim as Type2Auth material
  (Python semantics) rather than being mapped back to an 11-character serial.
- Error type names (`ValueError` / `RuntimeError` / `TimeoutError`) and exit
  codes (1 on failure) match the Python version.
