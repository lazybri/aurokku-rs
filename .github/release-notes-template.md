# auRokku <VERSION> — <RELEASE DATE>

<!--
Release-notes template.

Usage: copy this file to docs/release-notes-v<VERSION>.md, replace every
<PLACEHOLDER>, fill in the measured hashes when the artifacts exist, and paste
the result into the GitHub Release body. Do not invent hashes: keep the TBD
placeholder (and the note that says when it will be filled) until the published
artifacts have been downloaded and hashed.
-->

## Highlights

- <Main user-visible change 1>
- <Main user-visible change 2>
- <Packaging / compliance / CI changes, if any>

## Assets

| Platform | File | SHA256 |
| --- | --- | --- |
| Windows x86_64 | `auroku-windows-x86_64-<VERSION>.zip` | `<sha256>` |
| Linux x86_64 | `auroku-linux-x86_64-<VERSION>.tar.gz` | `<sha256>` |
| macOS universal (arm64 + x86_64) | `auroku-macos-universal-<VERSION>.zip` | `<sha256>` |

Every package contains `LICENSE`, `LICENSES/LGPL-3.0.txt`,
`LICENSES/LGPL-2.1.txt` and `THIRD-PARTY-NOTICES.md`; the Windows package may
additionally carry `LICENSE-LGPL-3.0.txt`.

## Installation

1. Download the archive for your platform from the Assets table above.
2. Verify its SHA256:
   - Linux/macOS: `shasum -a 256 <file>`
   - Windows: `Get-FileHash -Algorithm SHA256 <file>`
3. Extract it and run:

```bash
# Linux / macOS
./auroku-linux-x86_64 --op status --vid 0482 --pid 0a5c
./auroku-gui-linux-x86_64          # Linux Qt GUI (needs the system Qt 6 runtime)
./auroku --op status --vid 0482 --pid 0a5c   # macOS universal CLI
open ./auroku-gui.app              # macOS Qt GUI bundle
```

```bat
:: Windows
auroku.exe --op status --vid 0482 --pid 0a5c
auroku-gui-windows-x86_64\auroku-gui.exe   :: Qt DLLs are bundled next to it
```

## Upgrade

<How to replace a previous version; state files / driver state that are kept>

## Uninstall

<How to remove the extracted files and optional state directory>

## Drivers (Windows / WinUSB)

The device is claimed by `usbser.sys` (a COM port) and libusb cannot open it
until WinUSB is bound to it. The release binaries bundle the installer:

```bat
auroku.exe --op install-driver --vid 04C5 --pid 1270
auroku.exe --op restore-driver --vid 04C5 --pid 1270
```

- The Qt GUI exposes the same action as a **WinUSB driver** card (install /
  restore / restore-on-exit), with the current service and `driver.log` path.
- Installing needs one standard UAC prompt; the elevated helper is the release
  binary itself, so the prompt shows auRokku.
- Zadig or a signed INF are supported alternatives; see
  [docs/windows-driver.md](docs/windows-driver.md).

## Licensing

- auRokku is licensed under **GPL-3.0-only**; see `LICENSE`. There is no
  warranty.
- The complete **Corresponding Source** is the source tree at
  <https://github.com/lazybri/aurokku-rs/tree/<VERSION>> (all tags:
  <https://github.com/lazybri/aurokku-rs/tags>). It contains the Qt/CMake
  files, Rust build scripts and workflow used for the release.
- Third-party components:
  - **Qt 6** (Core, Gui, Widgets; dynamic link) — LGPL-3.0; replace the shipped
    Qt DLLs/frameworks (or the system Qt 6 on Linux) to relink. Source:
    <https://code.qt.io>; text: `LICENSES/LGPL-3.0.txt`.
  - **libusb** 1.0.27 via `rusb` (static link) — LGPL-2.1; source:
    <https://github.com/libusb/libusb>; text: `LICENSES/LGPL-2.1.txt`.
  - **libwdi** 1.5.1, pinned commit
    `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` (static link in the Windows
    release) — LGPL-3.0; source: <https://github.com/pbatard/libwdi>; build
    pipeline: <https://github.com/lazybri/aurokku-rs/tree/libwdi-static>.
  - Rust crates — mainly MIT / Apache-2.0; complete list in `Cargo.lock`.
- The GUI uses system fonts; icons are hand-drawn originals. Relinking
  instructions are in `THIRD-PARTY-NOTICES.md`.

## Verification

- CI run: <https://github.com/lazybri/aurokku-rs/actions/runs/<RUN_ID>>
- Builds: Windows (MSVC, static CRT + static libwdi + Qt GUI), Linux (static
  musl CLI + Qt GUI), macOS universal CLI + native Qt GUI.
- <Any smoke-test / static-link / package assertions worth calling out>

## Known issues

- **Unsigned binaries.** The releases are not code-signed. Windows SmartScreen
  and macOS Gatekeeper may warn on first launch; on macOS the binaries carry
  only an ad-hoc signature. Verify the SHA256 above before running.
- <Existing limitation 1 (e.g. Linux GUI requires the Qt 6 runtime)>
- <Existing limitation 2>

## Credits

- Thanks to Yuu for the original `auRokku.py`.
- Qt 6 by the Qt project; libwdi by Pete Batard and contributors; libusb by the
  libusb project; the Rust ecosystem crates listed in `Cargo.lock`.
