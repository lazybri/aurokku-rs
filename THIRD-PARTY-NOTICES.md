# Third-Party Notices / 第三方组件声明

This file states the licence of auRokku itself and lists the third-party
components that are distributed with its release artifacts. The English text is
authoritative; a short Chinese summary is included at the end.

## 1. Project licence

auRokku is free software distributed under the **GNU General Public License,
version 3 only (`GPL-3.0-only`)**. The complete licence text is the repository
root [`LICENSE`](LICENSE). There is **no warranty**, to the extent permitted by
law.

The `license` field in `Cargo.toml` previously said `MIT`; that was a
metadata mistake and has been corrected to `GPL-3.0-only`. The repository root
`LICENSE` has always been the GPLv3 text, and that licence governs the project.
This correction does not relicense any third-party component listed below.

### Corresponding Source / 对应源码

The complete Corresponding Source of every release is the source tree at the
Git tag that matches the release:

- this release: <https://github.com/lazybri/aurokku-rs/tree/v0.3.6>
- all tags: <https://github.com/lazybri/aurokku-rs/tags>

The source tree contains the complete build scripts and the GitHub Actions
workflow (`.github/`) used to produce the published binaries; no private
patches or additional source files are required. The vendored libusb source is
fetched automatically by `rusb`'s `vendored` feature when building.

## 2. Third-party components

| Component | Version / pin | Licence | Use in the release |
| --- | --- | --- | --- |
| libusb (via `rusb` / `libusb1-sys`, vendored) | libusb 1.0.27 (`libusb1-sys` 0.7.0, `rusb` 0.9.4) | LGPL-2.1 (or later) | statically linked into all binaries |
| libwdi (Windows only) | 1.5.1, upstream commit `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` | LGPL-3.0 | statically linked into the Windows release binaries |
| Rust crates | see `Cargo.lock` | mostly MIT / Apache-2.0 (see §2.3) | compiled into the binaries |
| egui default fallback fonts | via `epaint_default_fonts` 0.33.3 | OFL-1.1 / Ubuntu Font Licence 1.0 / MIT (see §3) | embedded fallback fonts in the GUI |
| Icons | — | original work | hand-drawn, no third-party icon assets |

This branch (the egui/eframe front-end) does **not** link Qt. The `qt6` branch
ships a Qt 6 GUI and lists Qt in its own `THIRD-PARTY-NOTICES.md`.

### 2.1 libusb — LGPL-2.1

`rusb` is used with the `vendored` feature, which compiles the libusb C
library bundled by `libusb1-sys` 0.7.0 (libusb 1.0.27). The resulting library is
**statically linked** into the CLI and GUI on Linux, Windows and macOS.

- Source: <https://github.com/libusb/libusb>
- Licence text: [`LICENSES/LGPL-2.1.txt`](LICENSES/LGPL-2.1.txt)
- libusb is used unmodified.

### 2.2 libwdi — LGPL-3.0 (Windows release only)

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

### 2.3 Rust crates — MIT / Apache-2.0 and other permissive licences

The complete dependency graph and exact versions are recorded in
[`Cargo.lock`](Cargo.lock); per-crate licence texts are published on crates.io
and in each crate's source package. The direct dependencies are:

| Crate | Licence |
| --- | --- |
| `clap` 4.6 (CLI parsing) | MIT OR Apache-2.0 |
| `serde_json` 1.0 (JSON output) | MIT OR Apache-2.0 |
| `md-5` 0.10 / `hex` 0.4 | MIT OR Apache-2.0 |
| `rusb` 0.9.4 | MIT |
| `libusb1-sys` 0.7.0 (native link) | MIT |
| `eframe` / `egui` / `ecolor` / `epaint` 0.33 (GUI) | MIT OR Apache-2.0 |
| `rustix` 1.x (Linux UID check) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |

Notable transitive dependencies include `winit` and `glutin` (Apache-2.0),
`wgpu` (MIT OR Apache-2.0), `windows-sys` (MIT OR Apache-2.0),
`libloading` (ISC), `tiny-skia` (BSD-3-Clause), `foldhash` / `zlib-rs`
(Zlib), `unicode-ident` (MIT OR Apache-2.0 AND Unicode-3.0), plus MIT,
BSD-2-Clause, BSL-1.0, CC0-1.0 and Unlicense licensed helper crates such as
`unicode-segmentation`, `byteorder-lite` and `clipboard-win`. All of these
are permissive and compatible with GPL-3.0-only.

### 2.4 Upstream protocol reference — GPL-3.0 (source only)

The pinned upstream Python protocol implementation is vendored under
[`docs/upstream/`](docs/upstream/README.md) for protocol comparison: upstream
<https://github.com/saga0324/auRokku>, commit
`3b1eccf01eb8429b98aa43696fb47673a21241bd`, GPL-3.0. It is **not** compiled,
linked or distributed with any release artifact; the upstream licence text is
kept at [`docs/upstream/LICENSE`](docs/upstream/LICENSE). Both projects are
GPL-3.0 and the vendored copy is unmodified (SHA256 recorded in its README).

## 3. Fonts and icons

At runtime the GUI prefers a system CJK font (Noto Sans CJK, Microsoft YaHei,
PingFang, …) and can be pointed at a font file with `AUROKU_GUI_FONT`. In
addition, the egui front-end carries the default `egui` fallback fonts supplied
by the `epaint_default_fonts` crate:

- Ubuntu-Light — Ubuntu Font Licence 1.0
  ([`LICENSES/Ubuntu-font-1.0.txt`](LICENSES/Ubuntu-font-1.0.txt))
- Hack — MIT / Bitstream Vera
  ([`LICENSES/Hack-font-notice.txt`](LICENSES/Hack-font-notice.txt))
- NotoEmoji — SIL OFL 1.1 ([`LICENSES/OFL-1.1.txt`](LICENSES/OFL-1.1.txt))
- emoji-icon-font — MIT
  ([`LICENSES/emoji-icon-font-MIT.txt`](LICENSES/emoji-icon-font-MIT.txt))

All application icons are hand-drawn original work; no third-party icon set or
image asset is bundled.

## 4. Relinking the LGPL components / 重新链接 LGPL 组件

The project as a whole is GPL-3.0-only and its complete Corresponding Source is
public. To relink either LGPL library, rebuild from the matching source tag:

- **libusb (LGPL-2.1):** replace the vendored libusb sources under
  `libusb1-sys` 0.7.0 with your modified libusb, then rebuild the complete
  program from the release tag (`cargo build --release --features gui --bins`,
  or the scripts in `.github/scripts/`). The build scripts in the release tag
  reproduce the published binaries.
- **libwdi (LGPL-3.0, Windows):** rebuild libwdi from the pinned upstream
  commit using the public `libwdi-static` pipeline
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

- 本项目整体以 **GPL-3.0-only** 发布，完整许可证见根目录 `LICENSE`；
  对应源码（Corresponding Source）见与发布版本同名的 Git tag，
  例如 <https://github.com/lazybri/aurokku-rs/tree/v0.3.6>。
- 第三方组件：libusb 1.0.27（LGPL-2.1，静态链接）、libwdi 1.5.1
  （LGPL-3.0，仅 Windows 发布版静态链接）、Rust crates（主要为
  MIT / Apache-2.0，完整清单见 `Cargo.lock`）、egui 内置回退字体
  （OFL-1.1 / Ubuntu Font Licence / MIT）。图标均为手绘原创。
- LGPL 组件的许可证文本在 `LICENSES/` 目录，并随每个发布包分发。
  要重新链接 libusb / libwdi，可按上文从公开源码与构建脚本重新构建。
- 本软件不提供任何担保。
