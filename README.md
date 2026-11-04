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
- Type2Auth 52/53 authentication with automatic fallback: start without base authentication and retry with base 52/53 if authentication fails (`--type2` forces base auth)
- GUI hotplug refresh: after unplug/replug or WinUSB install/restore, the device picker and the selected VID/PID/serial target are re-scanned and re-selected automatically
- Brand-tolerant SN parsing: 11-character serials, 16 hex NV10077 bytes, and 12-character USB serials that append one extra digit to the printed serial (the trailing digit is trimmed)
- Optional JSON output and verbose USB frame logs
- egui + eframe cross-platform GUI (Material 3, Chinese/English UI, Wayland / Windows / macOS)
- Resident WinUSB driver card on Windows: status, install / restore, log path, restore-on-exit, and a test-only Ports-class policy to block `usbser.sys` from taking over a device on its first connection (with PnP fallback so driverless devices stay visible)

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

On Windows the Overview page always shows a **resident WinUSB management card**:
it reports the device's current function driver service (`WINUSB`, `usbser`, …),
installs or restores the WinUSB driver (one UAC prompt), shows and copies the
driver log path, and can restore the original driver automatically when the GUI
exits. The card appears without waiting for a driver error.

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
  `.github/scripts/build-linux.sh` builds the static musl CLI and the host-glibc GUI in one
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
- **Permissions**: the Linux GUI runs unprivileged and requests device-scoped
  USB access after selection. On Windows the device is claimed by `usbser.sys`
  (it shows up as "USB Serial Device" / COMx) and libusb cannot open it until a
  WinUSB driver is installed — use the resident WinUSB card on the Overview page,
  or see [docs/windows-driver.md](docs/windows-driver.md) for Zadig / wdi-simple /
  signed-INF instructions. For test machines you can also block `usbser.sys` from
  taking over a device on its first connection with
  [docs/windows-disable-usbser.md](docs/windows-disable-usbser.md).

### Linux USB permissions

The GUI never runs as root and does not require a VID/PID-specific udev rule.
Scan and select the intended device in the GUI; only then does it request
polkit authorization to grant the logged-in user read/write ACL access to that
device's `/dev/bus/usb/BBB/DDD` node. The privileged helper rechecks the
selected VID, PID, bus and address before applying the ACL, and only changes
that USB node. On normal GUI shutdown, the GUI requests polkit authorization
once to restore the previous ACL (or remove the entry it added) for all devices
authorized during that session. If authorization is canceled or the process
does not shut down cleanly, the ACL remains until the device node is removed.

The Linux `acl` package (providing `getfacl` and `setfacl`) and polkit must be installed.
Canceling authorization leaves the GUI unprivileged and USB operations disabled
for that device. The GUI refuses to start as root.

The GUI’s Overview / UIM Lock / PIN & ICCID / Probe / Batch / Session / Console
pages cover every CLI `--op` operation. The console page shows USB TX/RX logs and
the full JSON result.

## Building

Requires a Rust toolchain (<https://rustup.rs>). `rusb` uses vendored libusb, so
a C compiler is also required (`gcc`/`clang` on Linux/macOS, MSVC Build Tools or
MinGW-w64 on Windows).

Builds run **only in GitHub Actions**: the repository ships no local build
entry points any more, and the workflow builds, verifies and publishes every
release. The scripts under `.github/scripts/` are invoked by CI only; artifacts
go to `dist/`. “Static” here refers to Rust/C runtime dependencies; whether
system libraries and the graphics stack can be linked statically is determined
by the OS:

| Platform | CI build | Artifacts |
| --- | --- | --- |
| Linux (static CLI + host glibc GUI) | `.github/scripts/build-linux.sh` | `dist/auroku-linux-x86_64` (static ELF), `dist/auroku-gui-linux-x86_64` (glibc) |
| Windows (static CRT + bundled libwdi) | `.github/scripts/build-windows.ps1` | `dist\auroku.exe`, `dist\auroku-gui.exe`, `dist\auroku-windows-<arch>.zip` |
| macOS (universal) | `.github/scripts/build-macos.sh` | `dist/auroku`, `dist/auroku-gui` (arm64 + x86_64) |
| macOS (native arch) | `.github/scripts/build-macos.sh --native` | `dist/auroku`, `dist/auroku-gui` |

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

The Windows job additionally inspects the PE import table with
`dumpbin /dependents` and fails the build if either binary imports a dynamic C
runtime (`vcruntime140*.dll`, `msvcp140*.dll`, `ucrtbase.dll`,
`api-ms-win-crt-*.dll`, `libgcc_s_*.dll`, …). Only OS DLLs are allowed, so the
static-CRT property cannot regress silently.

### Static linking verification

Verify the static Linux CLI with the following commands (tested on x86_64):

```bash
file dist/auroku-linux-x86_64
# ELF 64-bit LSB executable ... statically linked
readelf -d dist/auroku-linux-x86_64 | grep NEEDED   # no output
readelf -l dist/auroku-linux-x86_64 | grep INTERP   # no output
ldd dist/auroku-linux-x86_64                        # not a dynamic executable
```

`.github/scripts/build-linux.sh` performs these checks after the build and exits on failure.

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
`.github/scripts/build-linux.sh` is recommended: it prepares the UAPI headers and verifies static
linkage automatically.

### Cross-compiling Linux (x86_64 static musl)

On Windows you can cross-compile with Zig, without WSL / Docker:

```powershell
# One-time setup: install Zig, extract it to %USERPROFILE%\tools\zig-<version>,
# or set the ZIG environment variable / pass -ZigPath path\to\zig.exe
.\.github/scripts/windows-cross-build-linux.ps1
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
chmod +x .github/scripts/build-macos.sh
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

`.github/workflows/build.yml` has two build modes:

- **Push a commit (any branch)** → **debug build**: unoptimised binaries with
  debug symbols for Windows, Linux (static musl) and the macOS universal
  binary. They are uploaded as workflow artifacts named `*-debug` and are
  **never** published to the Releases page.
- **Manual dispatch, mode = `release`** → release build: same three platforms,
  optimised, and the workflow creates/publishes a GitHub Release. Enter the
  version (e.g. `v0.3.1`; a missing `v` is added automatically) and optionally
  mark it prerelease/draft.
- **Manual dispatch, mode = `debug`** → same debug build as a commit push.
- **Pull request** → only `cargo fmt --check`, `cargo clippy -D warnings` and
  `cargo test`.

Set `AUROKU_BUILD_DEBUG=1` (or pass `--debug` / `-Debug` to the build scripts)
to reproduce a debug build locally.

The Windows job consumes a **static libwdi** built once by the reusable pipeline
on the orphan `libwdi-static` branch (pinned by the immutable
`libwdi-1.5.1-x64-r1` tag) and links it into both binaries, so the shipped tools
can install the WinUSB driver without an external `wdi-simple.exe`. The build
recipe and LGPL compliance notes live in
[docs/windows-driver.md](docs/windows-driver.md).

Artifacts (each package contains both `auroku` and `auroku-gui`):

- `auroku-windows-x86_64-v<version>.zip` (plus `LICENSE-LGPL-3.0.txt`)
- `auroku-linux-x86_64-v<version>.tar.gz` (static musl CLI + host-glibc GUI)
- `auroku-macos-universal-v<version>.zip` (arm64 + x86_64)

Debug builds append `-debug` to the file name, e.g.
`auroku-windows-x86_64-v0.3.1-debug.zip`.

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

# Windows: install / restore the WinUSB driver (release builds bundle libwdi;
# local builds without it fall back to wdi-simple.exe)
auroku --op install-driver --vid 04c5 --pid 1270
auroku --op restore-driver --vid 04c5 --pid 1270

# JSON / verbose log / explicit SN / Type2Auth
auroku --op status --vid 0482 --pid 0a5c --json
auroku --op status --vid 0482 --pid 0a5c --verbose
auroku --op status --vid 0482 --pid 0a5c --sn SKYKA077394
auroku --op status --vid 0482 --pid 0a5c --sn 947307414B594B53 --type2
# 12-character USB serials with one extra trailing digit are auto-trimmed:
auroku --op status --vid 0482 --pid 0a5c --sn SCAEE0359010
```

## Relationship to the Python version

The original upstream protocol implementation is vendored, pinned and unmodified
under [`docs/upstream/`](docs/upstream/README.md)
(<https://github.com/saga0324/auRokku>, commit
`3b1eccf01eb8429b98aa43696fb47673a21241bd`) as the protocol baseline. It is not
compiled, linked or shipped in release artifacts.

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
- Some brands report the printed 11-character serial with one extra trailing
  digit in the USB iSerialNumber: a Casio CAY01 label reads `SCAEE035901`, while
  the descriptor returns `SCAEE0359010`. When the first 11 characters form a
  valid standard serial and the 12th character is a decimal digit, the parser
  drops that trailing digit and uses the normal 11-character layout (5 ASCII
  chars + hex tail, stored reversed). The canonical SN shown by the tool is the
  printed 11-character serial. 11-character serials and 16-hex inputs are
  unchanged, and truncated/malformed serials are still rejected.
- Error type names (`ValueError` / `RuntimeError` / `TimeoutError`) and exit
  codes (1 on failure) match the Python version.

## License

auRokku is free software licensed under the **GNU General Public License,
version 3 only (`GPL-3.0-only`)**. The complete licence text is
[`LICENSE`](LICENSE). There is **no warranty**, to the extent permitted by law.

The complete **Corresponding Source** of a release is the source tree at the
matching Git tag, for example
<https://github.com/lazybri/aurokku-rs/tree/v0.3.7> (all tags:
<https://github.com/lazybri/aurokku-rs/tags>). The tag contains the build
scripts and GitHub Actions workflow that produce the released binaries.

Release packages also contain [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
and the third-party licence texts:

- [LICENSES/LGPL-3.0.txt](LICENSES/LGPL-3.0.txt) — libwdi (LGPL-3.0, static
  link in the Windows release)
- [LICENSES/LGPL-2.1.txt](LICENSES/LGPL-2.1.txt) — libusb (LGPL-2.1, static
  link on all platforms)
- font licence texts in [`LICENSES/`](LICENSES/) for the egui fallback fonts

libusb and libwdi are statically linked LGPL libraries. Both can be relinked by
rebuilding the program from the public source tag with a modified library; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) §4 for the procedure.
