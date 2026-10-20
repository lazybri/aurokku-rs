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
- Qt 6 Widgets GUI (native Qt styling, Chinese/English UI, light/dark themes) driven by the same Rust backend through a C ABI

## GUI front-end (Qt 6 / Widgets)

`auroku-gui` is a C++ Qt 6 Widgets front-end that calls the same Rust backend
through a small C ABI bridge crate (`crates/auroku-ffi`). The UI uses standard
Qt widgets and a light colour layer on top of them (navigation rail, cards,
buttons, switches, segmented buttons, status chips, toast notifications and
confirmation dialogs) with light and dark palettes and a Chinese/English
interface.

- **Architecture**: `crates/auroku-ffi` exposes `auroku_scan_devices` and
  `auroku_run_task` as a shared library (`auroku_ffi.dll`, `libauroku_ffi.so`
  or `libauroku_ffi.dylib`). Requests and responses are JSON envelopes, so the
  C++ side never models Rust types. Every USB call runs on a dedicated
  `QThread`, so the window never blocks on libusb transfers or timeouts.
- **Language**: the selector in the navigation rail switches between 简体中文
  and English; `--lang=en|zh` overrides the first-start choice. The initial
  language follows the system locale (`zh*` → Chinese, otherwise English). If
  no CJK font is available the UI falls back to English automatically.
- **Theme**: the moon/sun button switches between light and dark; the dark
  theme is the default and `--light` starts in the light theme.

Build requirements: Qt 6.5+, CMake 3.21+ and Ninja, plus the usual Rust
toolchain. CMake builds the Rust C ABI library automatically.

```bash
# Configure & build (Linux / Windows / macOS)
cmake -S gui -B build/gui -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build/gui

# Run
./build/gui/auroku-gui
```

On Windows pass `-DCMAKE_PREFIX_PATH=C:/Qt/6.8.3/mingw_64` (or `msvc2022_64`);
`build-windows.ps1` discovers Qt automatically and packages the Qt runtime next
to the executable.

Developer flags: `--smoke-test` (start, then quit), `--screenshot=FILE` and
`--page=N` (0–6) allow headless UI checks with `QT_QPA_PLATFORM=offscreen`.

Platform notes:

- **Linux**: requires the Qt 6 runtime (`qt6-base`); for native Wayland also
  install `qt6-wayland` (otherwise Qt uses XWayland). `libauroku_ffi.so` must
  stay next to the GUI binary. USB access usually needs root or a matching udev
  rule. The GUI is a normal dynamic Qt application and cannot be fully static.
- **Windows**: Qt DLLs and plugins are deployed next to `auroku-gui.exe` by
  `windeployqt`; no separate Qt installation is required.
- **macOS**: `build-macos.sh` produces `dist/auroku-gui.app` (native
  architecture) and runs `macdeployqt` when available.
- **Fonts**: a CJK-capable family is selected at start-up (Microsoft YaHei /
  Noto Sans CJK / PingFang, …). Override it with
  `AUROKU_GUI_FONT=/path/to/font.ttf`.
- **Permissions**: same as the CLI; a libusb driver may be required on Windows.

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
  the display, try `QT_QPA_PLATFORM=xcb` (requires XWayland), or configure a
  udev rule and run unprivileged.
- On pure Wayland sessions (no `DISPLAY`) automatic elevation is skipped,
  because a root client usually cannot connect to the compositor. Use a udev
  rule, or run with `QT_QPA_PLATFORM=xcb` to elevate through XWayland.
- Security note: running the whole process as root increases the attack surface.
  If you do not need automatic elevation, skip polkit and configure a udev rule
  for the device.

The GUI’s Overview / UIM Lock / PIN & ICCID / Probe / Batch / Session / Console
pages cover every CLI `--op` operation. The console page shows USB TX/RX logs and
the full JSON result.

## Building

Requires a Rust toolchain (<https://rustup.rs>). `rusb` uses vendored libusb, so
a C compiler is also required (`gcc`/`clang` on Linux/macOS, MSVC Build Tools or
MinGW-w64 on Windows). The Qt GUI additionally needs Qt 6.5+, CMake 3.21+ and
Ninja.

Each platform has a local build script; artifacts go to `dist/`. The Linux,
Windows and macOS native scripts build **both the CLI and the GUI**. “Static”
here refers to Rust/C runtime dependencies; whether system libraries and the
graphics stack can be linked statically is determined by the OS:

| Platform | Command | Artifacts |
| --- | --- | --- |
| Linux (static CLI + Qt 6 GUI) | `chmod +x build-linux.sh && ./build-linux.sh` | `dist/auroku-linux-x86_64` (static ELF), `dist/auroku-gui-linux-x86_64` (Qt 6) |
| Windows (static CRT) | `.\build-windows.ps1` | `dist\auroku.exe`, `dist\auroku-gui-windows-<arch>\auroku-gui.exe` (+ Qt runtime), `dist\auroku-windows-<arch>.zip` |
| macOS (universal CLI + native GUI) | `chmod +x build-macos.sh && ./build-macos.sh` | `dist/auroku` (arm64 + x86_64), `dist/auroku-gui.app` |
| macOS (native arch) | `./build-macos.sh --native` | `dist/auroku`, `dist/auroku-gui.app` |

Static linking status:

| Artifact | Static status |
| --- | --- |
| Linux CLI (`auroku`) | **Fully static**: musl + static CRT, no `DT_NEEDED`, no `PT_INTERP` |
| Windows CLI (`auroku.exe`) | Static CRT (`crt-static`); system DLLs such as `kernel32`/`user32` are provided by Windows and cannot be static |
| macOS CLI | libSystem and system frameworks must be dynamic (macOS limitation); Rust dependencies and libusb are static |
| Linux GUI (`auroku-gui-linux-x86_64`) | Dynamically linked Qt 6 application; ship `libauroku_ffi.so` next to it; **cannot** be fully static |
| Windows GUI (`auroku-gui.exe`) | Qt 6 DLLs and plugins are bundled next to the executable |
| macOS GUI (`auroku-gui.app`) | Qt 6 frameworks are deployed into the bundle by `macdeployqt` |

The Linux GUI is a regular dynamically linked Qt 6 application; the static musl
target only applies to the CLI. Install the Qt 6 runtime (`qt6-base`) or ship the
Qt libraries alongside the binary.

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

Windows CLI (GNU build; the MSVC build links the CRT statically as well):

```powershell
objdump -p dist\auroku.exe | Select-String 'DLL Name'
# Expected: KERNEL32.dll, msvcrt.dll, ntdll.dll, bcryptprimitives.dll and
# api-ms-win-core-synch-*.dll — all shipped by Windows.
```

`build-windows.ps1` performs this check automatically and fails if a MinGW, Qt
or FFI DLL leaks into the CLI. The Qt GUI is intentionally **not** static: it
loads Qt 6 and `auroku_ffi.dll` at runtime. The FFI DLL itself links Rust,
libusb and the C runtime statically and only imports Windows system DLLs.

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

This cross script only ships the static CLI: the Qt GUI needs a full Linux Qt 6
sysroot and a C++ toolchain. Build the GUI on Linux with `./build-linux.sh` (or
download the Linux archive from the release artifacts).

### Building the macOS universal binary (arm64 + x86_64)

Run on macOS (with Xcode Command Line Tools installed):

```bash
chmod +x build-macos.sh
./build-macos.sh
```

The script builds the CLI for `aarch64-apple-darwin` and `x86_64-apple-darwin`,
combines both slices with `lipo`, applies an ad-hoc signature, and then builds
the Qt 6 GUI bundle for the current machine architecture.

Output: `dist/auroku` (universal CLI) and `dist/auroku-gui.app` (native Qt GUI).

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

Artifacts (each package contains the CLI and the Qt 6 GUI):

- `auroku-windows-x86_64.zip` (static-CRT CLI + GUI folder with the Qt runtime)
- `auroku-linux-x86_64.tar.gz` (static musl CLI + Qt 6 GUI + `libauroku_ffi.so`)
- `auroku-macos-universal.zip` (universal CLI + `auroku-gui.app`)

The Linux GUI is built on `ubuntu-22.04` against the Qt 6.8 SDK installed by the
workflow; install the Qt 6 runtime on the target system (or bundle the libraries
yourself). The CLI in the same package remains fully static.

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
