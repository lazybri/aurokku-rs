# auRokku v0.3.2-qt6 — 2026-11-04

Release commit: `44b44fe` (`qt6`). Tag: `v0.3.2-qt6`.

> **Status: Draft Release.** This release is currently kept as a GitHub draft
> so the Qt 6 GUI can be smoke-tested further before it is published. The
> assets and SHA256 values below were produced by CI run
> [41748412910](https://github.com/lazybri/aurokku-rs/actions/runs/41748412910).

## Highlights

- **Qt 6 Widgets GUI.** The front-end is a native C++ Qt 6 application driven by
  the same Rust backend through the `crates/auroku-ffi` C ABI (now **ABI 2**),
  with light/dark palettes and a Chinese/English UI. Overview / UIM Lock /
  PIN & ICCID / Probe / Batch / Session / Console pages cover every CLI
  operation.
- **WinUSB driver card (ABI 2).** The Qt GUI exposes the driver lifecycle in a
  dedicated card: install / restore buttons, a “restore on exit” opt-in, the
  current service, and the copyable `driver.log` path. A task that fails with
  `DriverError` jumps to the card and is retried automatically after a
  successful install. The GUI links the Rust backend statically, so there is no
  `auroku_ffi.dll` to deploy.
- **libwdi build decoupled from the product CI.** The Windows static libwdi is
  built once by the reusable, tag-pinned pipeline on the orphan
  `libwdi-static` branch (immutable tag `libwdi-1.5.1-x64-r1`, upstream
  libwdi 1.5.1, commit
  `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602`) and consumed as the
  `libwdi-static` artifact. No WDK download, upstream clone or msbuild in the
  product workflow. The Windows binaries still link libwdi statically, so
  driver installation needs no external `wdi-simple.exe`.
- **Build/CI policy unified with `main`.** Versioned artifacts
  (`auroku-<platform>-v<version>`), the shared `libwdi` job, debug builds on
  pushes, and one reusable `libwdi-static` pipeline for both branches.
- **GPLv3 compliance aligned.** The project licence is confirmed as
  **GPL-3.0-only** (root `LICENSE`). The stale `license = "MIT"` values in
  `Cargo.toml` and `crates/auroku-ffi/Cargo.toml` were corrected, and every
  package now ships `LICENSE`, `THIRD-PARTY-NOTICES.md`,
  `LICENSES/LGPL-3.0.txt` and `LICENSES/LGPL-2.1.txt`. See **Licensing**.

## Assets

| Platform | File | SHA256 |
| --- | --- | --- |
| Windows x86_64 | `auroku-windows-x86_64-v0.3.2-qt6.zip` | `f7f22aaa60287cf6643e505198eae5e69204966a919f6e7189ce7f3e9d4863dc` |
| Linux x86_64 | `auroku-linux-x86_64-v0.3.2-qt6.tar.gz` | `d5efbd21f0f861fc9ecb739687bf8bbb47d0a6722b8b2eb87230b3c65917729d` |
| macOS universal (arm64 + x86_64) | `auroku-macos-universal-v0.3.2-qt6.zip` | `dd42bf0dcea1b06f4c10d33c3b905b4cd1a886a05fbc2b1b70861154dd633bb5` |

These hashes were measured on the artifacts of CI run 41748412910. Re-verify
after download:

```bash
# Linux / macOS
shasum -a 256 auroku-windows-x86_64-v0.3.2-qt6.zip
```

```powershell
# Windows
Get-FileHash -Algorithm SHA256 .\auroku-windows-x86_64-v0.3.2-qt6.zip
```

Package contents:

- **Windows zip** — `auroku.exe`, `auroku-gui-windows-x86_64\` (Qt GUI, Qt
  DLLs and plugins, `auroku.exe` for UAC), `LICENSE`,
  `THIRD-PARTY-NOTICES.md`, `LICENSES/` and `LICENSE-LGPL-3.0.txt`.
- **Linux tar.gz** — static musl `auroku-linux-x86_64`, Qt 6
  `auroku-gui-linux-x86_64`, `LICENSE`, `THIRD-PARTY-NOTICES.md` and
  `LICENSES/`.
- **macOS zip** — universal `auroku` CLI, `auroku-gui.app` (Qt frameworks
  deployed by `macdeployqt`), `LICENSE`, `THIRD-PARTY-NOTICES.md` and
  `LICENSES/`.

## Installation

1. Download and verify the archive (see the SHA256 table above).
2. Extract it.
3. Run the CLI or the Qt GUI:

```bat
:: Windows
auroku.exe --op status --vid 0482 --pid 0a5c
auroku-gui-windows-x86_64\auroku-gui.exe
```

```bash
# Linux (requires the system Qt 6 runtime, e.g. qt6-base)
./auroku-linux-x86_64 --op status --vid 0482 --pid 0a5c
./auroku-gui-linux-x86_64
```

```bash
# macOS
./auroku --op status --vid 0482 --pid 0a5c
open ./auroku-gui.app
```

## Upgrade

Replace the extracted files with the new archive. Driver state under
`%LOCALAPPDATA%\auroku` on Windows is kept. On Linux/macOS the tools are
self-contained (the Linux GUI uses the system Qt 6 runtime).

## Uninstall

Delete the extracted files. On Windows, optionally remove
`%LOCALAPPDATA%\auroku` (override with `AUROKU_STATE_DIR`) and restore the
original driver with
`auroku.exe --op restore-driver --vid 04C5 --pid 1270`.

## Drivers (Windows / WinUSB)

The device enumerates as “USB Serial Device” (COMx) because Windows binds
`usbser.sys` to the whole CDC device. libusb can only open it after WinUSB is
bound to that device. The Qt GUI's **WinUSB driver** card and the CLI offer the
same operations:

```bat
auroku.exe --op install-driver --vid 04C5 --pid 1270
auroku.exe --op install-driver --vid 04C5 --pid 1270 --restore-on-exit
auroku.exe --op restore-driver --vid 04C5 --pid 1270
```

- One standard UAC prompt; the elevated helper is the release binary itself
  (the GUI folder ships `auroku.exe` for this), so the prompt shows auRokku.
- Restoring removes the device node, so Windows re-enumerates and re-binds
  `usbser`.
- Zadig and a signed INF are supported alternatives — see
  [docs/windows-driver.md](docs/windows-driver.md).

## Licensing

- auRokku is released under **GPL-3.0-only**; the full text is `LICENSE`.
  There is no warranty, to the extent permitted by law.
- **Corresponding Source:** the source tree at the matching tag,
  <https://github.com/lazybri/aurokku-rs/tree/v0.3.2-qt6>. It contains the
  Qt/CMake files, Rust build scripts and workflow used for this release.
- Third-party licences:
  - **Qt 6.8.3** (Core, Gui, Widgets; **dynamic link**) — **LGPL-3.0**.
    Windows/macOS packages bundle the Qt runtime; Linux uses the system Qt 6.
    Replace the Qt DLLs/frameworks to relink; source <https://code.qt.io>; text
    `LICENSES/LGPL-3.0.txt`.
  - **libusb** 1.0.27 (vendored through `rusb` 0.9.4 / `libusb1-sys` 0.7.0,
    statically linked) — **LGPL-2.1**; source
    <https://github.com/libusb/libusb>; text `LICENSES/LGPL-2.1.txt`.
  - **libwdi** 1.5.1, commit
    `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` (statically linked in the
    Windows release) — **LGPL-3.0**; source
    <https://github.com/pbatard/libwdi>; build pipeline
    <https://github.com/lazybri/aurokku-rs/tree/libwdi-static>; text
    `LICENSES/LGPL-3.0.txt` (and `LICENSE-LGPL-3.0.txt`).
  - **Rust crates** — mainly MIT / Apache-2.0; complete list in
    `Cargo.lock`.
  - The Qt GUI uses system fonts; icons are hand-drawn original work.
- **Relinking the LGPL libraries:** Qt can be relinked by replacing the shipped
  DLLs/frameworks; libusb/libwdi require rebuilding from the source tag with the
  modified library. Details: `THIRD-PARTY-NOTICES.md` §4.

## Verification

- CI run:
  <https://github.com/lazybri/aurokku-rs/actions/runs/41748412910>
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
  -D warnings` and `cargo test --workspace --all-targets` are green.
- Windows: offscreen GUI smoke test (`--smoke-test`), static-CRT/dependency
  assertion for the CLI, and libwdi licence packaging checks.
- Linux: static-ELF checks for the CLI, `libauroku_ffi` must not appear as a
  runtime dependency of the GUI, offscreen GUI smoke test, and a tar-content
  assertion for the compliance files.
- macOS: offscreen GUI smoke test and a zip-content assertion for the
  compliance files.

## Known issues

- **Unsigned binaries / SmartScreen.** The release is not code-signed. Windows
  SmartScreen may warn on first launch; macOS binaries carry only an ad-hoc
  signature and Gatekeeper may require “Open Anyway”. Verify the SHA256 above
  before running.
- **The Qt GUI needs a Qt 6 runtime on Linux** (`qt6-base`); the Windows and
  macOS packages bundle it. The CLI in the same archive is fully static.
- **WinUSB driver is required on Windows** for USB operations; installing it
  removes the COM port (expected), and restoring removes the device node so
  Windows re-binds `usbser`.
- **Draft release:** the Qt 6 packaging and the driver card are still being
  validated on real hardware, so this release stays a draft until that check
  completes.
- `Cargo.toml` previously advertised MIT; that metadata bug is fixed in this
  release, but older release pages may still show the old value.

## Credits

- Thanks to Yuu for the original `auRokku.py`.
- Qt 6 by the Qt project; libwdi by Pete Batard and contributors; libusb by the
  libusb project; the Rust ecosystem crates listed in `Cargo.lock`.
