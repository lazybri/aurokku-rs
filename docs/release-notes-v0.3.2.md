# auRokku v0.3.2 — 2026-11-04

Release commit: `ee37a79` (`main`). Tag: `v0.3.2`.

## Highlights

- **libwdi build decoupled from the product CI.** The Windows static libwdi is
  now built once by the reusable, tag-pinned pipeline on the orphan
  `libwdi-static` branch and consumed as the `libwdi-static` artifact. The
  product workflow no longer downloads the WDK, clones upstream libwdi or calls
  msbuild; it pins the immutable tag `libwdi-1.5.1-x64-r1` (upstream libwdi
  1.5.1, commit `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602`). The Windows
  binaries still link libwdi statically, so driver installation needs no
  external `wdi-simple.exe`.
- **Versioned release artifacts.** Every package now uses the
  `auroku-<platform>-v<version>` naming scheme, matching the release dispatch
  and the upload globs:
  `auroku-windows-x86_64-v0.3.2.zip`,
  `auroku-linux-x86_64-v0.3.2.tar.gz`,
  `auroku-macos-universal-v0.3.2.zip`.
- **GPLv3 compliance aligned.** The project licence is confirmed as
  **GPL-3.0-only** (the root `LICENSE` is the GPLv3 text). The stale
  `license = "MIT"` value in `Cargo.toml` was corrected to
  `GPL-3.0-only`, and every package now ships `LICENSE`,
  `THIRD-PARTY-NOTICES.md`, `LICENSES/LGPL-3.0.txt` and
  `LICENSES/LGPL-2.1.txt`. See **Licensing** below.
- No product behaviour changes: the CLI/GUI protocol code and the egui
  front-end are unchanged in this release.

## Assets

| Platform | File | SHA256 |
| --- | --- | --- |
| Windows x86_64 | `auroku-windows-x86_64-v0.3.2.zip` | TBD |
| Linux x86_64 | `auroku-linux-x86_64-v0.3.2.tar.gz` | TBD |
| macOS universal (arm64 + x86_64) | `auroku-macos-universal-v0.3.2.zip` | TBD |

> SHA256 values are placeholders in this template: **fill them in after the
> artifacts are published** (download each asset and hash it). 发布后再回填
> SHA256。

Package contents:

- **Windows zip** — `auroku.exe`, `auroku-gui.exe`, `LICENSE`,
  `LICENSES/LGPL-3.0.txt`, `LICENSES/LGPL-2.1.txt`,
  `THIRD-PARTY-NOTICES.md`, plus `LICENSE-LGPL-3.0.txt` for the statically
  linked libwdi.
- **Linux tar.gz** — static musl `auroku-linux-x86_64`, host-glibc
  `auroku-gui-linux-x86_64`, `LICENSE`, `THIRD-PARTY-NOTICES.md` and
  `LICENSES/`.
- **macOS zip** — universal `auroku` CLI, universal `auroku-gui`, `LICENSE`,
  `THIRD-PARTY-NOTICES.md` and `LICENSES/`.

## Installation

1. Download and verify the archive (for example
   `Get-FileHash -Algorithm SHA256 auroku-windows-x86_64-v0.3.2.zip` or
   `shasum -a 256 ...`).
2. Extract it.
3. Run the CLI or GUI:

```bat
:: Windows
auroku.exe --op status --vid 0482 --pid 0a5c
auroku-gui.exe
```

```bash
# Linux / macOS
./auroku-linux-x86_64 --op status --vid 0482 --pid 0a5c
./auroku-gui-linux-x86_64          # Linux GUI (glibc 2.35+)
./auroku --op status --vid 0482 --pid 0a5c   # macOS
./auroku-gui                          # macOS GUI
```

## Upgrade

Replace the extracted files with the new archive. Configuration and driver
state under `%LOCALAPPDATA%\auroku` (Windows) are kept; on Linux/macOS the
tools are self-contained.

## Uninstall

Delete the extracted files. On Windows, optionally remove the stored state
directory `%LOCALAPPDATA%\auroku` (set `AUROKU_STATE_DIR` to relocate it) and
restore the original driver with
`auroku.exe --op restore-driver --vid 04C5 --pid 1270`.

## Drivers (Windows / WinUSB)

The device enumerates as “USB Serial Device” (COMx) because Windows binds
`usbser.sys` to the whole CDC device. libusb can only open it after WinUSB is
bound to that device. The release binaries bundle the installer:

```bat
auroku.exe --op install-driver --vid 04C5 --pid 1270
auroku.exe --op restore-driver --vid 04C5 --pid 1270
```

- One standard UAC prompt; the elevated helper is the release binary itself, so
  the prompt shows auRokku. The GUI exposes the same action in its driver card.
- Zadig and a signed INF remain supported alternatives — see
  [docs/windows-driver.md](docs/windows-driver.md).

## Licensing

- auRokku is released under **GPL-3.0-only**; the full text is `LICENSE`.
  There is no warranty, to the extent permitted by law.
- **Corresponding Source:** the source tree at the matching tag,
  <https://github.com/lazybri/aurokku-rs/tree/v0.3.2>. It contains the build
  scripts and GitHub Actions workflow used for this release.
- Third-party licences:
  - **libusb** 1.0.27 (vendored through `rusb` 0.9.4 / `libusb1-sys` 0.7.0,
    statically linked) — **LGPL-2.1**; source
    <https://github.com/libusb/libusb>; text `LICENSES/LGPL-2.1.txt`.
  - **libwdi** 1.5.1, commit
    `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` (statically linked in the
    Windows release) — **LGPL-3.0**; source
    <https://github.com/pbatard/libwdi>; build pipeline
    <https://github.com/lazybri/aurokku-rs/tree/libwdi-static>; text
    `LICENSES/LGPL-3.0.txt` (and `LICENSE-LGPL-3.0.txt`).
  - **Rust crates** — mainly MIT / Apache-2.0; the complete dependency list is
    `Cargo.lock`. The egui front-end also embeds the default egui fallback
    fonts (OFL-1.1 / Ubuntu Font Licence 1.0 / MIT), whose texts are under
    `LICENSES/`.
  - Icons are hand-drawn original work; no third-party icon assets.
- **Relinking the LGPL libraries:** with the source tag above, rebuild with a
  modified libusb, or (Windows) rebuild libwdi from the pinned public pipeline
  and point `LIBWDI_LIB_DIR` at it before
  `.github/scripts/build-windows.ps1`. Details:
  `THIRD-PARTY-NOTICES.md` §4.

## Verification

- CI run:
  <https://github.com/lazybri/aurokku-rs/actions/runs/41741178316>
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
  and `cargo test --all-targets` are green.
- Windows: `dumpbin /dependents` static-CRT assertion (only OS DLLs).
- Linux: static-ELF checks (`file`, no `PT_INTERP`, no `DT_NEEDED`,
  `ldd`).
- Packaging: the Linux tar step asserts `LICENSE`, `THIRD-PARTY-NOTICES.md`
  and both LGPL texts are present; the Windows build fails if any compliance
  file is missing from the archive inputs.

## Known issues

- **Unsigned binaries / SmartScreen.** The release is not code-signed. Windows
  SmartScreen may warn on first launch; macOS binaries carry only an ad-hoc
  signature and Gatekeeper may require “Open Anyway”. Verify the SHA256 above
  before running.
- **Linux GUI requires glibc 2.35+** (built on Ubuntu 22.04); the CLI in the
  same archive is fully static and has no runtime dependencies.
- **WinUSB driver is required on Windows** for USB operations; installing it
  removes the COM port (expected), and restoring removes the device node so
  Windows re-binds `usbser`.
- `Cargo.toml` previously advertised MIT; that metadata bug is fixed in this
  release, but older release pages may still show the old value.

## Credits

- Thanks to Yuu for the original `auRokku.py`.
- libwdi by Pete Batard and contributors; libusb by the libusb project; Qt 6 by
  the Qt project; the Rust ecosystem crates listed in `Cargo.lock`.
