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
./auroku --op status --vid 0482 --pid 0a5c
```

```bat
:: Windows
auroku.exe --op status --vid 0482 --pid 0a5c
```

<GUI-specific launch notes for this branch>

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

- The GUI exposes the same action as a **WinUSB driver** card.
- Installing needs one standard UAC prompt; the elevated helper is the release
  binary itself, so the prompt shows auRokku.
- Zadig or a signed INF are supported alternatives; see
  [docs/windows-driver.md](docs/windows-driver.md).

## Licensing

- auRokku is licensed under **GPL-3.0-only**; see `LICENSE`. There is no
  warranty.
- The complete **Corresponding Source** is the source tree at
  <https://github.com/lazybri/aurokku-rs/tree/<VERSION>> (all tags:
  <https://github.com/lazybri/aurokku-rs/tags>).
- Third-party components:
  - **Qt 6** (only on the `qt6` branch; dynamic link) — LGPL-3.0; replace the
    shipped Qt DLLs/dylibs to relink. Source: <https://code.qt.io>. The main
    branch's egui front-end does not link Qt.
  - **libusb** 1.0.27 via `rusb` (static link) — LGPL-2.1; source:
    <https://github.com/libusb/libusb>.
  - **libwdi** 1.5.1, pinned commit
    `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` (static link in the Windows
    release) — LGPL-3.0; source: <https://github.com/pbatard/libwdi>; build
    pipeline: <https://github.com/lazybri/aurokku-rs/tree/libwdi-static>.
  - Rust crates — mainly MIT / Apache-2.0; complete list in `Cargo.lock`.
- The archive ships `THIRD-PARTY-NOTICES.md`, `LICENSES/LGPL-3.0.txt` and
  `LICENSES/LGPL-2.1.txt`; relinking instructions are in
  `THIRD-PARTY-NOTICES.md`.

## Verification

- CI run: <https://github.com/lazybri/aurokku-rs/actions/runs/<RUN_ID>>
- Builds: Windows (MSVC, static CRT + static libwdi), Linux (static musl CLI +
  host-glibc GUI), macOS universal.
- <Any smoke-test / static-link / packaging assertions worth calling out>

## Known issues

- **Unsigned binaries.** The releases are not code-signed. Windows SmartScreen
  and macOS Gatekeeper may warn on first launch; on macOS the binaries carry
  only an ad-hoc signature. Verify the SHA256 above before running.
- <Existing limitation 1 (e.g. Linux GUI needs glibc 2.35+)>
- <Existing limitation 2>

## Credits

- Thanks to Yuu for the original `auRokku.py`.
- <Other contributors / upstream projects to acknowledge>
