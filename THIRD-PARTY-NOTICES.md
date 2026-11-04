# Third-Party Notices / 第三方组件声明

This file states the licence of auRokku (Qt 6 branch) and lists the third-party
components distributed with its release artifacts. The English text is
authoritative; a short Chinese summary is included at the end.

## 1. Project licence

auRokku is free software distributed under the **GNU General Public License,
version 3 only (`GPL-3.0-only`)**. The complete licence text is the repository
root [`LICENSE`](LICENSE). There is **no warranty**, to the extent permitted by
law.

The `license` fields in `Cargo.toml` and `crates/auroku-ffi/Cargo.toml`
previously said `MIT`; that was a metadata mistake and has been corrected to
`GPL-3.0-only`. The repository root `LICENSE` has always been the GPLv3 text,
and that licence governs the project. This correction does not relicense any
third-party component listed below.

### Corresponding Source / 对应源码

The complete Corresponding Source of every release is the source tree at the
Git tag that matches the release:

- this release: <https://github.com/lazybri/aurokku-rs/tree/v0.3.4-qt6>
- all tags: <https://github.com/lazybri/aurokku-rs/tags>

The source tree contains the complete Qt/CMake build files, the Rust build
scripts and the GitHub Actions workflow (`.github/`) used to produce the
published binaries; no private patches or additional source files are required.
The vendored libusb source is fetched automatically by `rusb`'s `vendored`
feature when building.

## 2. Third-party components

| Component | Version / pin | Licence | Use in the release |
| --- | --- | --- | --- |
| Qt 6 (Core, Gui, Widgets + deployed plugins) | 6.8.3 | LGPL-3.0 | dynamically linked GUI; Windows/macOS packages bundle the Qt runtime, Linux uses the system Qt 6 |
| libusb (via `rusb` / `libusb1-sys`, vendored) | libusb 1.0.27 (`libusb1-sys` 0.7.0, `rusb` 0.9.4) | LGPL-2.1 (or later) | statically linked into the CLI and into the GUI backend |
| libwdi (Windows only) | 1.5.1, upstream commit `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` | LGPL-3.0 | statically linked into the Windows release binaries |
| Rust crates | see `Cargo.lock` | mostly MIT / Apache-2.0 (see §2.4) | compiled into the CLI and the GUI backend |
| Icons | — | original work | hand-drawn, no third-party icon assets |

### 2.1 Qt 6 — LGPL-3.0

The Windows release links the Qt 6 Widgets GUI against the dynamically loaded
Qt DLLs; the macOS app bundle contains Qt frameworks deployed by
`macdeployqt`; the Linux binary links the system Qt 6 runtime. Qt 6 open-source
editions are available under **LGPL-3.0** (or GPL-3.0). Qt is used unmodified.

- Source: <https://code.qt.io>
- Licence text: [`LICENSES/LGPL-3.0.txt`](LICENSES/LGPL-3.0.txt)
- Relinking: replace the shipped Qt DLLs/dylibs (Windows/macOS) or install a
  different Qt 6 runtime (Linux) with a compatible build; no rebuild of auRokku
  is required. If you distribute a modified Qt, ship the Qt source or a written
  offer as required by the LGPL.
- The Qt runtime also contains Qt's own third-party components; their notices
  are part of the Qt distribution (see the Qt source tree).

### 2.2 libusb — LGPL-2.1

`rusb` is used with the `vendored` feature, which compiles the libusb C
library bundled by `libusb1-sys` 0.7.0 (libusb 1.0.27). The resulting library is
**statically linked** into the CLI and into the GUI backend on Linux, Windows
and macOS.

- Source: <https://github.com/libusb/libusb>
- Licence text: [`LICENSES/LGPL-2.1.txt`](LICENSES/LGPL-2.1.txt)
- libusb is used unmodified.

### 2.3 libwdi — LGPL-3.0 (Windows release only)

The Windows release statically links libwdi 1.5.1 so that the binaries can
install the WinUSB driver without an external `wdi-simple.exe`. libwdi is built
once by the public reusable pipeline on the orphan `libwdi-static` branch of
this repository, pinned by the immutable tag `libwdi-1.5.1-x64-r1`; the
upstream source is pinned to commit
`9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602`.

- Source: <https://github.com/pbatard/libwdi> (pin above)
- Build recipe: <https://github.com/lazybri/aurokku-rs/tree/libwdi-static>
- Licence text: [`LICENSES/LGPL-3.0.txt`](LICENSES/LGPL-3.0.txt), also
  included in the Windows archive as `LICENSE-LGPL-3.0.txt`
- libwdi is used unmodified.

### 2.4 Rust crates — MIT / Apache-2.0 and other permissive licences

The complete dependency graph and exact versions are recorded in
[`Cargo.lock`](Cargo.lock); per-crate licence texts are published on crates.io
and in each crate's source package. The direct dependencies are:

| Crate | Licence |
| --- | --- |
| `clap` 4.6 (CLI parsing) | MIT OR Apache-2.0 |
| `serde_json` 1.0 (JSON envelopes) | MIT OR Apache-2.0 |
| `md-5` 0.10 / `hex` 0.4 | MIT OR Apache-2.0 |
| `rusb` 0.9.4 | MIT |
| `libusb1-sys` 0.7.0 (native link) | MIT |
| `rustix` 1.x (Linux UID check) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `libwdi-sys` 0.1.0 (git tag `libwdi-1.5.1-x64-r1`) | MIT |

Notable transitive dependencies include `anstream` / `anstyle` (MIT OR
Apache-2.0), `windows-sys` (MIT OR Apache-2.0), `indexmap`
(Apache-2.0 OR MIT), `foldhash` (Zlib), `memchr` (Unlicense OR MIT),
`unicode-ident` (MIT OR Apache-2.0 AND Unicode-3.0) and `generic-array`
(MIT). All of these are permissive and compatible with GPL-3.0-only.

## 3. Fonts and icons

The Qt GUI uses **system fonts** at runtime (Microsoft YaHei / Noto Sans CJK /
PingFang, …); no font files are bundled. Override the family with
`AUROKU_GUI_FONT`. All application icons are hand-drawn original work; no
third-party icon set or image asset is bundled.

## 4. Relinking the LGPL components / 重新链接 LGPL 组件

The project as a whole is GPL-3.0-only and its complete Corresponding Source is
public. The LGPL components can be relinked as follows:

- **Qt 6 (LGPL-3.0):** dynamically linked. Replace the Qt DLLs in
  `auroku-gui-windows-x86_64\` (Windows), the frameworks inside
  `auroku-gui.app` (macOS), or the system Qt 6 libraries (Linux) with your own
  compatible Qt build; the auRokku binary does not need to be rebuilt.
- **libusb (LGPL-2.1):** statically linked. Replace the vendored libusb sources
  under `libusb1-sys` 0.7.0 with your modified libusb, then rebuild the
  complete program from the release tag
  (`cargo build --release --features static-libwdi --bin auroku`, plus the
  Qt GUI through CMake / `.github/scripts/`).
- **libwdi (LGPL-3.0, Windows):** statically linked. Rebuild libwdi from the
  pinned upstream commit using the public `libwdi-static` pipeline
  (<https://github.com/lazybri/aurokku-rs/tree/libwdi-static>), place the new
  `libwdi.lib` plus `COPYING-LGPL` in a directory and set
  `LIBWDI_LIB_DIR` to it, then run
  `.github/scripts/build-windows.ps1`. The script links that static library
  and packages its licence text.

Because all Corresponding Source and build scripts are published, no additional
object files or written offer are required.

## 5. No warranty

The software and the third-party components listed above are provided **“as
is”**, without warranty of any kind, express or implied. See the GPLv3 text
([`LICENSE`](LICENSE)) for the full disclaimer.

## 6. 中文摘要

- 本项目（qt6 分支）整体以 **GPL-3.0-only** 发布，完整许可证见根目录
  `LICENSE`；对应源码见与发布版本同名的 Git tag，例如
  <https://github.com/lazybri/aurokku-rs/tree/v0.3.4-qt6>。
- 第三方组件：Qt 6.8.3（LGPL-3.0，动态链接；Windows/macOS 包内附带 Qt 运行时，
  Linux 依赖系统 Qt 6）、libusb 1.0.27（LGPL-2.1，静态链接）、libwdi 1.5.1
  （LGPL-3.0，仅 Windows 发布版静态链接）、Rust crates（主要为
  MIT / Apache-2.0，完整清单见 `Cargo.lock`）。图标均为手绘原创，字体使用
  系统字体。
- LGPL 组件的许可证文本在 `LICENSES/` 目录，并随每个发布包分发。
  Qt 可直接替换 DLL/dylib 重新链接；libusb / libwdi 可按上文从公开源码与构建
  脚本重新构建。
- 本软件不提供任何担保。
