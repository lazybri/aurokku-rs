# libwdi 静态库构建解耦设计（草案）

> **状态**：草案（draft）。本文是只读分析 + 方案设计，不含任何代码改动；实施需 Lead 评审后再动仓库。
> **分析基线**：main @ `b219bd5`（`v0.3.1` = `77e1064` 之后一个提交）；qt6 @ `51ff33b`（`v0.2.0-qt6` = 附注标签 `515ea6e` -> 提交 `e21f816`）；共同祖先 `777c2d2`（`v0.1.0` / `v0.1.0-qt6`）。
> **远端**：https://github.com/lazybri/aurokku-rs.git（public），git 2.54，Windows + PowerShell。
> **命令约定**：一律 `git -C <repo> ...`、路径用正斜杠；本文所有 git 操作均为只读（log/show/diff/merge-tree/show-ref）。
> **日期**：2026-11-04。
> **引用格式**：`file:line` 均指 main 工作树 `C:/Users/Administrator/Documents/proj/aurokku-rs` 或 qt6 工作树 `C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6` 的当前内容。

---

## 0. 结论摘要（TL;DR）

1. **现状耦合点很窄**：libwdi 的编译逻辑几乎全部内联在 `.github/workflows/build.yml:111-157` 这一个 step 里；它与产品代码只通过两个契约相连——环境变量 `LIBWDI_LIB_DIR`（`build.yml:157`）和 Cargo feature `static-libwdi`（`Cargo.toml:16` + `.github/scripts/build-windows.ps1:114-121`）。Rust 侧唯一接口是 `crates/auroku-driver-sys/build.rs:8-30`。
2. **可搬走的是"生成 libwdi.lib 的全部步骤 + 校验 + 许可 + 构建文档"**（WDK 下载/`msiexec /a` 解包、libwdi v1.5.1 克隆、`config.h` 裁剪、vcxproj 工程引用删除、3 个 msbuild 目标 + `libwdi_static`、产物/符号/静态 CRT 校验、`COPYING-LGPL`、分支自测）。
3. **必须留在产品仓库的是**：feature 开关与可选依赖、`src/driver.rs` 的产品语义（UAC 策略、state/log、`pnputil` 恢复）、最终 exe 的链接与 PE 校验、LGPL 文本进 zip、版本 pin 与 Release 打包。
4. **`crates/auroku-driver-sys` 分两阶段搬迁**：Cut1 只搬构建（crate 暂留 main，最小风险）；Cut2 在 qt6 接入前把 crate 改名为 `libwdi-sys` 搬到新分支，两个产品分支都用 git tag 依赖，消除绑定重复。
5. **推荐主方案**：**B（预编译 release asset，SHA256 pin 在 main/qt6 仓库内）作为二进制载体 + C（cargo git tag 依赖 `libwdi-sys`）作为代码载体**（C 在第二阶段启用）。
6. **回退方案**：A（同仓库跨分支可复用工作流 `workflow_call`，每次从源码构建）。**不推荐** D（git submodule / subtree）。
7. **关键修正（会影响 qt6 设计）**：qt6 的 Rust 后端是被 CMake 当 **staticlib** 用的（`gui/CMakeLists.txt:59-69`、`140`）。Rust 的 `staticlib` **不会**把 `libwdi.lib` 和系统导入库打进去（rustc 只在 bin/cdylib 链接阶段消费 `cargo:rustc-link-lib`；staticlib 由消费者补齐）。因此 qt6 的最终链接**必须显式**加 `libwdi.lib` + 19 个系统库，而 main 的 bin 不需要。这一点与"仅 Rust 静态链接进 dll/静态库"的直觉不同，见 `5。
8. **最大风险**：LGPLv3 静态链接义务（必须提供可重链接途径 + 打包 `COPYING-LGPL`）与预编译产物的供应链可信度（用 main 仓库内 review 过的 SHA256 pin 作信任锚），其次是上游 vcxproj/Toolset 与 `windows-latest` 镜像漂移、上游 tag 可变。
9. **附带收益**：当前 PR 根本不构建 Windows（`build.yml:70` 的 `if: github.event_name != 'pull_request'`），libwdi 相关改动无法在 PR 验证；解耦成预编译产物后，PR 只需要下载 + 链接测试即可低成本补上这条门禁（详见 `7.2）。
10. **不能用"合并分支"来共享**：`git merge-tree --write-tree main qt6` 在 `.github/workflows/build.yml`、`Cargo.toml`、`Cargo.lock`、`src/lib.rs`、`README*.md` 等 10+ 文件上冲突（附录 A.3），所以共享只能走产物/独立分支，不能走 main↔qt6 合并。

---

## 1. 依赖清点：现在"生成 libwdi.lib 并让 Rust 链接成功"到底依赖什么

### 1.1 端到端链路

```text
[runner 工具]  microsoft/setup-msbuild@v2            -> msbuild.exe
[网络]        curl LinkID=253170                     -> wdk-redist.msi (~31 MB)
[runner]      msiexec /a TARGETDIR=$PWD\wdk          -> wdk\Windows Kits\8.0
[网络]        git clone --branch v1.5.1 libwdi       -> libwdi/
[脚本内联]    config.h 4 处 -replace                -> WinUSB/X64-only 配置
[msbuild]     detect_64build.vcxproj  (x64)          -> libwdi\build64.h
[msbuild]     installer_x64.vcxproj   (x64)          -> x64 payload 库
[msbuild]     embedder.vcxproj        (Win32)        -> 载荷嵌入器
[脚本内联]    删除 libwdi_static 的 <ProjectReference>
[msbuild]     libwdi_static.vcxproj   (x64)          -> libwdi\x64\Release\lib\libwdi.lib
[workflow]    断言 libwdi.lib 存在 + 写 $GITHUB_ENV   -> LIBWDI_LIB_DIR
[产品脚本]    build-windows.ps1 检测到变量            -> --features gui,static-libwdi
[cargo]       auroku-driver-sys/build.rs             -> -L native + -l static=libwdi + 19 dylib
[cargo/link]  auroku.exe / auroku-gui.exe            -> 静态链入
[产品脚本]    dumpbin /dependents                    -> 静态 CRT 校验
[产品脚本]    libwdi/COPYING-LGPL                    -> LICENSE-LGPL-3.0.txt 进 zip
```

### 1.2 有序步骤与现状位置（S0–S23）

> "搬迁判定"列：**搬** = 应进 `libwdi-static` 分支；**留** = 必须留在产品仓库；**混合** = 两边各承担一部分（见 `2）。

| # | 步骤 | 现状位置（file:line） | 具体内容 | 搬迁判定 |
| --- | --- | --- | --- | --- |
| S0 | 产品侧 feature/依赖接线 | `Cargo.toml:9-16`、`Cargo.toml:35-37`、`Cargo.toml:43-44`；`src/lib.rs:24` | `static-libwdi = ["dep:auroku-driver-sys"]`；`[target.'cfg(windows)'.dependencies]` 可选 path 依赖；`[workspace] members = ["crates/auroku-driver-sys"]`；`pub mod driver;` | 留 |
| S1 | 产品侧调用点 | `src/driver.rs:392-404`（静态后端）、`:425-444`（restore）、`:461-472`、`:476-495`、`:498-514`、`:520-587`；CLI/GUI 调用见 `src/lib.rs:2750-2768`、`src/bin/auroku-gui/worker.rs:166-175`、`src/bin/auroku-gui/app.rs:2790-2795` | UAC 只在 `WDI_ERROR_NEEDS_ADMIN` 时提权、提权自身 CLI、`pnputil` 清驱动包 + 移除设备节点、日志/状态文件 | 留 |
| S2 | 安装 MSBuild | `.github/workflows/build.yml:108-109` | `microsoft/setup-msbuild@v2`（`windows-x86_64` 矩阵项） | 搬（作为构建前置） |
| S3 | 下载 WDK redist MSI | `build.yml:120` | `curl.exe -sSL "https://go.microsoft.com/fwlink/p/?LinkID=253170" -o wdk-redist.msi` | 搬（改为可缓存 + SHA256 校验） |
| S4 | `msiexec /a` 解包 WDK | `build.yml:121`；文档 `docs/windows-driver.md:182` | `Start-Process msiexec.exe -Wait -ArgumentList '/a','wdk-redist.msi','/qn',"TARGETDIR=$PWD\wdk"`，只解包不安装、无需管理员 | 搬 |
| S5 | 克隆 libwdi 并 pin 版本 | `build.yml:122`；文档 `windows-driver.md:183` | `git clone --depth 1 --branch v1.5.1 https://github.com/pbatard/libwdi.git`；**tag 可变、未 pin SHA** | 搬（改为完整 commit SHA） |
| S6 | `config.h` 定义裁剪 | `build.yml:126-134` | 4 条正则替换：`#define OPT_M32`/`#define OPT_ARM` 注释掉（x64-only 载荷）；`LIBUSB0_DIR`/`LIBUSBK_DIR` 注释掉（只嵌 WinUSB 载荷） | 搬（改为 `patches/*.patch`） |
| S7 | 路径与 MSBuild 宏 | `build.yml:136-141` | `SolutionDir="<repo>\libwdi\"`；`WDK_DIR="<repo>/wdk/Windows Kits/8.0"`（**正斜杠**：`\a`、`\w`、`\W` 会被 MSVC 当转义）；`/p:Configuration=Release`、`/p:BuildMacros=WDK_DIR="..."` | 搬 |
| S8 | 构建 `detect_64build` | `build.yml:143-144` | `msbuild libwdi\libwdi\.msvc\detect_64build.vcxproj @common /p:Platform=x64`；注释说明它生成 `libwdi\build64.h` 供 embedder 包含 | 搬 |
| S9 | 构建 `installer_x64` | `build.yml:145` | 生成 amd64 WinUSB 载荷 | 搬 |
| S10 | 构建 `embedder` | `build.yml:146` | `/p:Platform=Win32`（该工程只有 Win32 配置） | 搬 |
| S11 | 删除 `libwdi_static` 的工程引用 | `build.yml:147-153` | 正则删除 `<ProjectReference>…</ProjectReference>`；原因（`:147-149`）：引用会按 `Release x64` 平台配置解析，而 `embedder/installer_x86` 只有 Win32，触发 **MSB8013** | 搬（改为 `patches/*.patch` + 断言删除条数） |
| S12 | 构建 `libwdi_static` | `build.yml:155` | `msbuild libwdi\libwdi\.msvc\libwdi_static.vcxproj @common /p:Platform=x64` | 搬 |
| S13 | 断言静态库存在 | `build.yml:156` | `if (-not (Test-Path "libwdi\x64\Release\lib\libwdi.lib")) { throw ... }` | 搬（加强为 `verify-libwdi.ps1`） |
| S14 | 导出 `LIBWDI_LIB_DIR` | `build.yml:157` | `"LIBWDI_LIB_DIR=$PWD\libwdi\x64\Release\lib"`，由 `Out-File` 追加到 `$env:GITHUB_ENV` | 搬（变成产物契约） |
| S15 | 调用产品打包脚本 | `build.yml:159-162` | `./.github/scripts/build-windows.ps1 -Target x86_64-pc-windows-msvc` | 留 |
| S16 | 条件启用 feature | `.github/scripts/build-windows.ps1:114-121` | 只有 `$env:LIBWDI_LIB_DIR` 存在且是目录时 `$features = 'gui,static-libwdi'`；否则仅 `gui`，回退外部 `wdi-simple.exe` | 留（契约不变） |
| S17 | 静态 CRT 编译标志 | `build-windows.ps1:101-106`；`.cargo/config.toml:5-6` | `RUSTFLAGS=-C target-feature=+crt-static`（脚本与 `.cargo/config.toml` 双保险） | 留 |
| S18 | `cargo build --locked` | `build-windows.ps1:135-136` | `cargo build @profileArgs --locked --target $Target --features $features --bins` | 留 |
| S19 | Rust 侧链接库 | `crates/auroku-driver-sys/build.rs:8-30` | `rerun-if-env-changed=LIBWDI_LIB_DIR`；缺失只 warn（`:10-16`）；`rustc-link-search=native={dir}`（`:18`）；`rustc-link-lib=static=libwdi`（`:19`）；19 个系统导入库（`:24-30`） | 混合（Cut1 留 / Cut2 搬） |
| S20 | 静态 CRT / PE 导入表校验 | `build-windows.ps1:39-91`、`:155-161` | `Find-Dumpbin`（`:46-55`，vswhere 或 PATH）；`Assert-StaticRuntime`（`:57-91`）：解析 `dumpbin /dependents`，禁止 `vcruntime*`/`msvcp140*`/`ucrtbase.dll`/`api-ms-win-crt-*`/`concrt140`/MinGW 运行库（`:78-85`） | 混合（库自身校验搬，产品 exe 校验留） |
| S21 | LGPL 许可打包 | `build-windows.ps1:163-171` | 从 `<repo>/libwdi/COPYING-LGPL` 复制为 `dist/LICENSE-LGPL-3.0.txt` 并加入 zip；**只有该文件存在才复制**（`:166`），否则静默跳过 | 混合（库提供许可文件，产品负责打进去） |
| S22 | 打包与发布 | `build-windows.ps1:173-175`；`build.yml:200-205`、`:207-243` | zip 名 `auroku-windows-<arch>-v<version><suffix>.zip`；`actions/upload-artifact@v4`；`softprops/action-gh-release@v2` | 留 |
| S23 | 文档 | `docs/windows-driver.md:34-79`、`:177-195`；`README.md:240-243`；`README.zh-CN.md:216-218` | 用户向说明 + 构建步骤 + 许可证声明 | 混合（构建细节迁到分支文档，产品文档保留"怎么用/许可"） |

### 1.3 Rust 侧链接面（S19 展开）

`crates/auroku-driver-sys/build.rs:24-30` 列了 **19** 个系统导入库，按出现顺序：

```text
setupapi, ole32, ntdll, advapi32, user32, gdi32, comctl32, crypt32, wintrust,
version, shlwapi, cfgmgr32, msi, shell32, oleaut32, uuid, comdlg32, winspool, userenv
```

绑定 crate 本体：`crates/auroku-driver-sys/Cargo.toml:8` `publish = false`；`src/lib.rs:7` `#![cfg(windows)]`；对外 API = `WDI_SUCCESS/WDI_ERROR_*` 常量（`:17-27`）、`WdiDeviceInfo`/`WdiOptions*`（`:30-74`）、`install_winusb`（`:175-254`）、`lookup_ids`（`:260-295`，内部）、`is_elevated`（`:136-161`）、`set_log_level`（`:164-169`）、`WdiError`（`:119-133`）。

> **重要**：这些 `rustc-link-lib` 只在 **bin / cdylib** 的最终链接阶段生效。`staticlib`（qt6 的 `auroku-ffi`）不会把 native 静态库打包进去，消费者必须自己补——见 `5。

### 1.4 外部依赖清单（decoupling 时要显式化）

| 外部依赖 | 现状引用 | 失效模式 |
| --- | --- | --- |
| WDK redist MSI（`LinkID=253170`） | `build.yml:120` | 微软 redirect 变更、MSI 下线、下载被墙 |
| libwdi 上游仓库 + tag `v1.5.1` | `build.yml:122` | tag 被移动/删除、上游重构 `msvc/` 布局 |
| Visual Studio / MSBuild / MSVC 工具集 | `build.yml:108-109` + `msbuild` | `windows-latest` 镜像升级后 `libwdi_static.vcxproj` 的 `PlatformToolset` 不可用 → MSB8020 |
| `dumpbin.exe`（VS 安装） | `build-windows.ps1:46-55` | 找不到时直接 throw（`:157`） |
| Git / cargo / PowerShell 7 | runner 预装 | 一般稳定 |
| 网络（`LinkID=253170` 与 `github.com/pbatard/libwdi`） | `build.yml:120`、`:122` | 每次 CI 都要访问两个外网 |

### 1.5 现状的 8 个断点（解耦要顺便修掉的）

| # | 断点 | 证据 | 影响 |
| --- | --- | --- | --- |
| D1 | 上游版本 pin 在**可变 tag** 上，不是 commit SHA | `build.yml:122` | 上游移动 tag 即静默改变发布产物，不可复现、不可审计 |
| D2 | `config.h`/vcxproj 的修改只存在于 workflow 的 `-replace` 正则里，没有 patch 文件 | `build.yml:126-134`、`:150-153` | 无法 review diff、无法在本地复现、上游文件一变就静默失配 |
| D3 | `COPYING-LGPL` 路径依赖"clone 到仓库根目录"这一副作用 | `build-windows.ps1:164` + `build.yml:122` | 本地构建/其他布局下 `Test-Path` 为假 → **静默不打许可文件**（`:166`），LGPL 合规变成 CI 的偶然行为 |
| D4 | 静态校验的 banned 列表不含 `libwdi.dll` / `libusb-1.0.dll` | `build-windows.ps1:78-85` 对比注释 `:42-44`（"must never mention ... a sibling DLL"） | 注释承诺比实现强；若哪天误链动态 libwdi，校验不会抓 |
| D5 | PR 不构建 Windows | `build.yml:70` | 改 `driver.rs`/`build.rs`/feature 的 PR 在合并前没有 Windows 编译门禁 |
| D6 | 构建文档与实现漂移 | `docs/windows-driver.md:177-191` 只写了 5 步、1 条 `msbuild`，实际 4 条 + setup-msbuild + 4 项 config.h 替换 + 工程引用删除 + `$GITHUB_ENV` 注入 | 新人按文档无法复现 |
| D7 | 每个引用分支各自下 31 MB MSI + 解包 + clone + 4 次 msbuild | `build.yml:120-155` | qt6 接入后成本翻倍；且受 D1/D2 影响两处漂移 |
| D8 | 搬 crate 必须同步改 `Cargo.lock`，否则 `--locked` 失败 | `Cargo.lock:452`、`:462-464`；`build-windows.ps1:136` | Cut2 迁移时必须提交 lock 变更，CI 不会自动生成 |

---

## 2. 解耦边界判断

### 2.1 三类归属

**A. 搬到 `libwdi-static` 分支（与产品无关）**

- WDK 下载 + `msiexec /a` 解包 + `Windows Kits/8.0` 定位；
- libwdi 上游 pin（完整 SHA）+ clone/fetch；
- `config.h` 裁剪（4 项）、vcxproj 工程引用删除 → 以 `patches/*.patch` 固化；
- `detect_64build/installer_x64/embedder/libwdi_static` 的 msbuild 调用与参数（`PlatformToolset` 可覆盖）；
- `libwdi.lib` 的存在性 / 符号 / `/MT` / SHA256 校验；
- `COPYING-LGPL` 的获取与随产物分发；
- 分支自测 workflow、`consume-smoke`、构建文档、`BUILD-INFO.json` 契约、Release asset 发布；
- （Cut2）`crates/libwdi-sys` 绑定与链接指令。

**B. 必须留在产品仓库**

- Cargo feature `static-libwdi` 与可选依赖声明（`Cargo.toml:9-16`、`:35-37`）；
- `src/driver.rs` 的全部产品语义（UAC 策略、state 文件、日志、`pnputil` 恢复路径、GUI 交互）；
- 产物**下载与校验**（哪个 libwdi 版本由产品决定）、`LIBWDI_LIB_DIR` 的注入；
- 最终 `auroku.exe`/`auroku-gui.exe` 的静态 CRT + PE 导入表校验；
- `LICENSE-LGPL-3.0.txt` 打进产品 zip、Release notes、版本号；
- `Cargo.lock` 的 pin、Release tag、PR 门禁。

**C. 共享契约（两边都要遵守）**

| 契约 | 内容 | 现状锚点 |
| --- | --- | --- |
| C1 产物契约 | zip 内固定布局：`lib/libwdi.lib`、`COPYING-LGPL`、`BUILD-INFO.json`、`SHA256SUMS`、`patches/`、`upstream-ref.txt` | 现状只有 `libwdi.lib`（`build.yml:156-157`） |
| C2 环境契约 | `LIBWDI_LIB_DIR` 指向含 `libwdi.lib` 的目录，语义不变 | `build.yml:157`、`build-windows.ps1:118`、`build.rs:10` |
| C3 crate 契约 | `libwdi-sys` 0.1.x；`WDI_*` 常量、`install_winusb`、`is_elevated`、`set_log_level`、`WdiError` 语义稳定；对应 libwdi 1.5.x ABI | `crates/auroku-driver-sys/src/lib.rs:17-133` |
| C4 feature 契约 | 产品 feature 名保持 `static-libwdi`，CI 命令保持 `--features static-libwdi`（main 额外带 `gui`） | `Cargo.toml:16`、`build-windows.ps1:119` |
| C5 版本契约 | 产物 tag：`libwdi-<upstream>-x64-r<N>`，`<upstream>` = `1.5.1`，`r<N>` = 我们的补丁/工具链修订号 | 现状无版本概念 |

### 2.2 `crates/auroku-driver-sys` 到底算不算"该搬走"？两种切法

**Cut1：只搬 CI 构建产物，crate 留在产品仓库**

- main/qt6 各自保留 `crates/auroku-driver-sys`（或 qt6 复制一份）；CI 只做"下载 `libwdi.lib` + `LIBWDI_LIB_DIR`"。
- 优点：产品仓库内改动最小（只删 `build.yml:111-157`、换成下载步骤）；crate 的 review 仍在产品 PR 流程里；无 cargo git 依赖 ⇒ 无 lockfile/网络/鉴权/离线问题；产物可以缓存或 vendor。
- 缺点：绑定代码在两个产品分支重复（qt6 需要一份）；libwdi 升级时 crate 的 `build.rs`/符号可能与产物不同步；"哪个 libwdi 版本配哪版绑定"没有单一事实源。
- 适用：第一阶段（1–2 天落地），或明确只在 main 使用 driver 能力。

**Cut2：连 bindings crate 一起搬到 `libwdi-static` 分支（改名 `libwdi-sys`）**

- 两个产品分支通过 `libwdi-sys = { git = "...", tag = "libwdi-1.5.1-x64-r1", optional = true }` 引用。
- 优点：绑定、链接指令、libwdi 版本、许可文件、补丁**单一事实源**；qt6 接入只需加依赖 + feature；`build.rs` 可以自己校验产物（SHA256/符号）；分支自测可覆盖"绑定 ↔ 产物"兼容性。
- 缺点：引入 Cargo git 源 ⇒ 必须提交 `Cargo.lock`（且 `--locked` 生效，`build-windows.ps1:136`）；首次/清缓存构建要联网 clone 该分支（可用 `Swatinem/rust-cache` 缓存 `~/.cargo/git/db`）；私仓要配 token；review 跨分支（新分支不是默认分支，PR 门禁需额外配置）；离线构建需 `cargo vendor`。
- 适用：目标态（qt6 确实要驱动安装能力时）。

**判断**：**算**——但**不要一次做完**。推荐 **Cut1 先行、Cut2 在 qt6 接入前完成**。理由：Cut1 能立刻把 WDK/MSBuild/clone 从两个产品 CI 里拿掉（最大收益、最小风险）；Cut2 的收益（去重）只在 qt6 也要 driver 时才兑现，而它带来的 lockfile/网络复杂度会拖慢第一步。

### 2.3 最小搬迁集 vs 最小保留集

- 最小搬迁集（Cut1 就能拿走）：`build.yml:111-157` 的全部 step 内容 + `build-windows.ps1:163-171` 里"许可来源"那一半 + `docs/windows-driver.md:177-191` 的构建细节。
- 最小保留集（main 侧改动量）：`build.yml` 里 1 个 `download+verify` step；`build-windows.ps1:114-121` 保持不动；`build-windows.ps1:163-171` 把 `libwdi\COPYING-LGPL` 换成 `<artifact>/COPYING-LGPL`；`docs/README` 改链接。

---

## 3. `libwdi-static` 分支设计

### 3.1 分支与标签策略

| 项 | 设计 | 理由 |
| --- | --- | --- |
| 分支名 | `libwdi-static` | 与任务命名一致，语义明确 |
| 创建方式 | **orphan 分支**：`git switch --orphan libwdi-static`（从 main 的最新提交开始但清空索引），首个提交只含本分支文件 | 不携带产品历史/代码，避免被误当成可合并分支；体积小；分支内容 = 独立构建工具 |
| 标签 | 每个发布版本一个 tag：`libwdi-1.5.1-x64-r1`；产物 asset 与 cargo git 引用**共用同一 tag** | 单一 rev；Cargo `tag` 与 Release tag 对齐，减少"两个版本号" |
| 保护（需管理员） | 分支保护：禁止直推、要求 PR + 1 review；tag 保护：禁止移动/删除 `libwdi-*` | 防 D1 类"tag 被移动"；tag 是供应链信任锚 |
| 提交规范 | 改动必须同时更新 `upstream/libwdi.ref`（SHA）与 `BUILD-INFO.json` 的生成参数；自测必须绿 | 可审计 |

> 说明：`docs/windows-driver.md:179-180` 声称"仓库不再提供本地构建入口"。新分支的 `tools/*.ps1` 不在产品仓库，因此该声明对产品仓库仍成立；产品仓库只多出"下载产物"这一步（见 `4.2）。

### 3.2 目录树与职责

```text
libwdi-static/
├── Cargo.toml                          # 纯 virtual workspace：members = ["crates/libwdi-sys"]（关键，见 3.4）
├── Cargo.lock                          # 锁定 crate 自身依赖（当前仅 std）
├── README.md                           # 分支用途、产物契约、消费方式（A/B/C 三种）
├── LICENSE                             # 本分支脚本/绑定 crate 的 MIT
├── LICENSES/
│   └── LGPL-3.0.txt                    # 上游 COPYING-LGPL 的校验副本（构建时与 clone 的比对）
├── .gitattributes                      # patches/* -text / *.ps1 eol=crlf 等，防止 patch 被 EOL 改写
├── .gitignore                          # .work/ dist/ target/
├── upstream/
│   └── libwdi.ref                      # 单行：v1.5.1 的完整 40-hex commit SHA（+ 注释行说明 tag）
├── crates/
│   └── libwdi-sys/
│       ├── Cargo.toml                  # name = "libwdi-sys"、links = "libwdi"、publish = false
│       ├── build.rs                    # LIBWDI_LIB_DIR → -L/-l + 19 系统库 + 产物校验
│       ├── src/lib.rs                  # 从 crates/auroku-driver-sys/src/lib.rs 改名迁移
│       └── tests/link_smoke.rs         # 链接期冒烟：cargo test --no-run 即验证 libwdi.lib 可链
├── tools/
│   ├── build-libwdi.ps1                # 完整构建：WDK → clone → patch → msbuild ×4 → stage → zip
│   ├── verify-libwdi.ps1               # 产物校验：存在/SHA256/符号/MT/许可/BUILD-INFO
│   └── apply-patches.ps1               # git apply --check + apply，失败即停
├── patches/
│   ├── 0001-config-win32-x64-only.patch       # OPT_M32/OPT_ARM + LIBUSB0_DIR/LIBUSBK_DIR
│   └── 0002-libwdi-static-drop-project-references.patch  # 删除 <ProjectReference>
├── docs/
│   ├── BUILDING.md                     # 与 build-libwdi.ps1 一一对应的步骤/参数/排错
│   ├── ARTIFACT.md                     # C1 产物契约 + BUILD-INFO.json schema
│   └── LGPL-COMPLIANCE.md             # 静态链接义务、可重链接材料清单
└── .github/workflows/
    ├── libwdi-static.yml               # 自测 + workflow_call + tag→Release asset
    └── consume-smoke.yml               # 用"局外人"方式消费产物（B）与 git 依赖（C）
```

职责要点：

- `tools/build-libwdi.ps1` 是**唯一**允许触碰上游工程文件的地方（通过 `patches/`），不再在 workflow 里内联 `-replace`（修 D2）。
- `patches/` 是构建输入的"源码级事实"：任何 `config.h`/vcxproj 改动都必须先成为 patch 文件。
- `docs/ARTIFACT.md` 定义 C1，两个产品仓库只依赖这份契约，不依赖分支内部实现。
- `consume-smoke.yml` 用"消费者视角"验证：下载 release asset、校验 hash、`cargo test --no-run` 链接通过——把"契约是否可用"变成 CI 门禁。

### 3.3 关键文件骨架（草案，需在实施时补全/验证）

#### 3.3.1 分支根 `Cargo.toml`（纯 workspace）

```toml
# libwdi-static 分支根清单：故意不使用 [package]，只做 virtual workspace。
#
# 为什么必须这样：产品仓库（main/qt6）会用
#   libwdi-sys = { git = "...", tag = "libwdi-1.5.1-x64-r1" }
# 引用本分支里的 crates/libwdi-sys。Cargo 解析 git 依赖时先读仓库根的
# Cargo.toml：根是 virtual workspace 时它会按 members 找到目标包；
# 若根里同时有 [package]（且名字不是 libwdi-sys），默认只会解析根包，
# 出现 no matching package named libwdi-sys found 的错误。
# 用 consume-smoke.yml 的 "外部空 crate + cargo metadata" 锁定该行为。
[workspace]
members = ["crates/libwdi-sys"]
resolver = "2"
```

#### 3.3.2 `crates/libwdi-sys/Cargo.toml`

```toml
[package]
name = "libwdi-sys"
version = "0.1.0"
edition = "2021"
rust-version = "1.88"
description = "Static libwdi 1.5.x bindings (WinUSB installer) built by the libwdi-static branch"
license = "MIT"
publish = false
# links 声明本 crate 提供 native 库 "libwdi"：同一依赖图里只允许一个提供者，
# 也便于 cargo 在错误信息里指出重复链接。
links = "libwdi"
build = "build.rs"

[lib]
name = "libwdi_sys"
path = "src/lib.rs"
```

#### 3.3.3 `crates/libwdi-sys/build.rs`（骨架）

```rust
//! Locate and link the static libwdi produced by the libwdi-static branch.
//!
//! 语义：
//! - LIBWDI_LIB_DIR 未设置      -> 只 warn（允许 cargo check / 离线元数据操作）
//! - LIBWDI_LIB_DIR 设置了但目录里没有 libwdi.lib -> 直接 panic（显式要求=失败要响亮）
//! - 成功 -> -L native + -l static=libwdi + 19 个系统导入库
//!
//! 注意：这些 -l 只对 bin/cdylib 的最终链接生效；staticlib 的消费者
//! （qt6 的 auroku-ffi -> CMake）必须自己补 libwdi.lib 和系统库。

use std::path::PathBuf;

const SYSTEM_LIBS: &[&str] = &[
    "setupapi", "ole32", "ntdll", "advapi32", "user32", "gdi32", "comctl32", "crypt32",
    "wintrust", "version", "shlwapi", "cfgmgr32", "msi", "shell32", "oleaut32", "uuid",
    "comdlg32", "winspool", "userenv",
];

fn main() {
    println!("cargo:rerun-if-env-changed=LIBWDI_LIB_DIR");

    let dir = match std::env::var_os("LIBWDI_LIB_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => {
            println!(
                "cargo:warning=LIBWDI_LIB_DIR is not set; the static libwdi will be missing \
                 at link time (see libwdi-static branch docs)"
            );
            return;
        }
    };

    let lib = dir.join("libwdi.lib");
    if !lib.is_file() {
        panic!(
            "LIBWDI_LIB_DIR={} does not contain libwdi.lib; \
             download the pinned libwdi-1.5.1-x64 release asset",
            dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=libwdi");
    for lib in SYSTEM_LIBS {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
```

#### 3.3.4 `crates/libwdi-sys/src/lib.rs`（骨架 + 迁移说明）

```rust
//! Minimal static bindings for libwdi 1.5.x (WinUSB driver installation).
//!
//! 迁移来源：产品仓库的 crates/auroku-driver-sys/src/lib.rs:1-305。
//! 迁移时只做三件事：
//!   1. crate 名 auroku_driver_sys -> libwdi_sys（文档/注释同步改名）；
//!   2. 保持 #![cfg(windows)] 与 #![allow(unsafe_code)]；
//!   3. 函数体逐字保留（本骨架只列 API 与不变量，实施时整段复制）。
#![cfg(windows)]
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

const WDI_WINUSB: i32 = 0;
const INF_NAME: &str = "usb_device.inf";
const WORK_DIR_NAME: &str = "auroku-wdi";

pub const WDI_SUCCESS: c_int = 0;
pub const WDI_ERROR_IO: c_int = -1;
pub const WDI_ERROR_INVALID_PARAM: c_int = -2;
pub const WDI_ERROR_NEEDS_ADMIN: c_int = -15;
pub const WDI_LOG_LEVEL_INFO: c_int = 1;
pub const WDI_LOG_LEVEL_NONE: c_int = 4;

#[repr(C)]
pub struct WdiDeviceInfo {
    pub next: *mut WdiDeviceInfo,
    pub vid: u16,
    pub pid: u16,
    pub is_composite: i32,
    pub mi: u8,
    pub desc: *mut c_char,
    pub driver: *mut c_char,
    pub device_id: *mut c_char,
    pub hardware_id: *mut c_char,
    pub compatible_id: *mut c_char,
    pub upper_filter: *mut c_char,
    pub driver_version: u64,
}

#[repr(C)]
pub struct WdiOptionsInstallDriver { /* 同 auroku-driver-sys/src/lib.rs:46-52 */ }
#[repr(C)]
pub struct WdiOptionsPrepareDriver { /* 同 :54-66 */ }
#[repr(C)]
pub struct WdiOptionsCreateList { /* 同 :68-74 */ }

extern "system" {
    fn wdi_create_list(list: *mut *mut WdiDeviceInfo, options: *mut WdiOptionsCreateList) -> c_int;
    fn wdi_destroy_list(list: *mut WdiDeviceInfo) -> c_int;
    fn wdi_prepare_driver(
        device_info: *mut WdiDeviceInfo, path: *const c_char,
        inf_name: *const c_char, options: *mut WdiOptionsPrepareDriver,
    ) -> c_int;
    fn wdi_install_driver(
        device_info: *mut WdiDeviceInfo, path: *const c_char,
        inf_name: *const c_char, options: *mut WdiOptionsInstallDriver,
    ) -> c_int;
    fn wdi_strerror(errcode: c_int) -> *const c_char;
    fn wdi_set_log_level(level: c_int) -> c_int;
}

pub struct WdiError { pub code: c_int, pub message: String }
impl std::fmt::Display for WdiError { /* :127-131 */ }
impl std::error::Error for WdiError {}

pub fn is_elevated() -> bool { /* 迁移自 :136-161，逐字 */ }
pub fn set_log_level(level: c_int) { /* :164-169 */ }
pub fn install_winusb(vid: u16, pid: u16) -> Result<(), WdiError> { /* :175-254 */ }
fn lookup_ids(vid: u16, pid: u16) -> (Option<CString>, Option<CString>) { /* :260-295 */ }
fn error_message(code: c_int) -> String { /* :297-305 */ }
```

`tests/link_smoke.rs`（链接期证明，不真正安装驱动）：

```rust
//! 这个测试不 mock 任何东西：cargo test --no-run 会把测试二进制链接到
//! libwdi.lib，从而证明产物存在、符号齐全、19 个系统库可解析。
#[test]
fn libwdi_symbols_are_linkable() {
    assert_eq!(libwdi_sys::WDI_SUCCESS, 0);
    assert_eq!(libwdi_sys::WDI_ERROR_NEEDS_ADMIN, -15);
    let _ = libwdi_sys::is_elevated; // 取函数地址 => 链接器必须解析符号
}
```

#### 3.3.5 `tools/build-libwdi.ps1`（完整骨架）

```powershell
<#
.SYNOPSIS
    从固定上游 SHA 构建 x64-only 静态 libwdi.lib（仅 WinUSB 载荷），产出可发布产物。
.DESCRIPTION
    1) 下载/缓存 WDK redist MSI，校验 SHA256，用 msiexec /a 解包（不安装、无需管理员）。
    2) clone libwdi 并 checkout 到 upstream/libwdi.ref 中的完整 commit SHA。
    3) 应用 patches/*.patch（替代旧 workflow 的内联正则）。
    4) msbuild: detect_64build(x64) -> installer_x64(x64) -> embedder(Win32) -> libwdi_static(x64)。
    5) 组装产物契约（lib/libwdi.lib、COPYING-LGPL、patches/、BUILD-INFO.json、SHA256SUMS）。
.PARAMETER LibwdiRef
    上游 commit SHA（默认读 upstream/libwdi.ref）。
.PARAMETER OutDir
    产物目录，默认 <repo>/dist。
.NOTES
    草案。未在 runner 实测；实施时必须先跑通并记录 BUILD-INFO.json。
#>
[CmdletBinding()]
param(
    [string]$LibwdiRef = '',
    [string]$WdkUrl = 'https://go.microsoft.com/fwlink/p/?LinkID=253170',
    [string]$WorkDir = '',
    [string]$OutDir = '',
    [switch]$KeepWorkDir
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $WorkDir) { $WorkDir = Join-Path $root '.work/libwdi' }
if (-not $OutDir)  { $OutDir  = Join-Path $root 'dist' }
New-Item -ItemType Directory -Force -Path $WorkDir, $OutDir | Out-Null

function Invoke-Native {
    param([string]$Exe, [string[]]$Arguments, [string]$What)
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# ---- 0) 解析并校验上游 ref（必须是完整 SHA，禁止 tag）----------------------
if (-not $LibwdiRef) {
    $LibwdiRef = (Get-Content (Join-Path $root 'upstream/libwdi.ref') -Raw).Trim()
}
if ($LibwdiRef -notmatch '^[0-9a-f]{40}$') {
    throw "LibwdiRef must be a full 40-hex commit SHA, got '$LibwdiRef'"
}

# ---- 1) WDK redist：下载（缓存）-> 校验 -> msiexec /a 解包 -------------------
$msi = Join-Path $WorkDir 'wdk-redist.msi'
if (-not (Test-Path $msi)) {
    Write-Host "==> downloading WDK redist MSI"
    Invoke-WebRequest -Uri $WdkUrl -OutFile $msi -MaximumRedirection 5
}
$msiSha = (Get-FileHash $msi -Algorithm SHA256).Hash.ToLowerInvariant()
if ($env:WDK_MSI_SHA256 -and $msiSha -ne $env:WDK_MSI_SHA256.ToLowerInvariant()) {
    throw "WDK MSI SHA256 mismatch: expected $env:WDK_MSI_SHA256, got $msiSha"
}
$wdkRoot = Join-Path $WorkDir 'wdk'
if (-not (Test-Path (Join-Path $wdkRoot 'Windows Kits/8.0'))) {
    Write-Host "==> extracting WDK redist with msiexec /a (no install, no admin)"
    $p = Start-Process msiexec.exe -Wait -PassThru -ArgumentList @(
        '/a', "`"$msi`"", '/qn', "TARGETDIR=`"$wdkRoot`""
    )
    if ($p.ExitCode -ne 0) { throw "msiexec /a failed with exit code $($p.ExitCode)" }
}
if (-not (Test-Path (Join-Path $wdkRoot 'Windows Kits/8.0'))) {
    throw "WDK extraction did not produce 'Windows Kits/8.0' under $wdkRoot"
}

# ---- 2) clone/checkout 上游（pin 到 SHA）-----------------------------------
$src = Join-Path $WorkDir 'libwdi'
if (-not (Test-Path (Join-Path $src '.git'))) {
    Invoke-Native git @('clone', '--no-checkout', 'https://github.com/pbatard/libwdi.git', $src) 'git clone'
}
Invoke-Native git @('-C', $src, 'fetch', '--depth', '1', 'origin', $LibwdiRef) 'git fetch'
Invoke-Native git @('-C', $src, 'checkout', '--force', $LibwdiRef) 'git checkout'
$head = (& git -C $src rev-parse HEAD).Trim()
if ($head -ne $LibwdiRef) { throw "libwdi checkout mismatch: expected $LibwdiRef, got $head" }

# ---- 3) 应用补丁（config.h 裁剪 + 工程引用删除）----------------------------
& (Join-Path $PSScriptRoot 'apply-patches.ps1') -Repo $src -PatchDir (Join-Path $root 'patches')

# ---- 4) msbuild ------------------------------------------------------------
$slnDir = (Resolve-Path $src).Path + '/'
# 正斜杠：MSVC 会把 \a、\w、\W 当转义序列，破坏 WDK_DIR。
$wdkDir = (Join-Path $wdkRoot 'Windows Kits/8.0') -replace '\', '/'
$macros = 'WDK_DIR="' + $wdkDir + '"'
$common = @('/m', '/p:Configuration=Release', "/p:SolutionDir=$slnDir", "/p:BuildMacros=$macros")

Invoke-Native -Exe 'msbuild' -Arguments (@("$src/libwdi/.msvc/detect_64build.vcxproj") + $common + @('/p:Platform=x64')) -What 'detect_64build'
Invoke-Native -Exe 'msbuild' -Arguments (@("$src/libwdi/.msvc/installer_x64.vcxproj") + $common + @('/p:Platform=x64')) -What 'installer_x64'
Invoke-Native -Exe 'msbuild' -Arguments (@("$src/libwdi/.msvc/embedder.vcxproj")      + $common + @('/p:Platform=Win32')) -What 'embedder'
Invoke-Native -Exe 'msbuild' -Arguments (@("$src/libwdi/.msvc/libwdi_static.vcxproj") + $common + @('/p:Platform=x64')) -What 'libwdi_static'

# ---- 5) 断言产物并按契约组装 ----------------------------------------------
$lib = Join-Path $src 'x64/Release/lib/libwdi.lib'
if (-not (Test-Path $lib)) { throw "libwdi.lib was not produced: $lib" }

$pkg = Join-Path $WorkDir 'pkg'
Remove-Item $pkg -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path (Join-Path $pkg 'lib'), (Join-Path $pkg 'patches') | Out-Null
Copy-Item $lib (Join-Path $pkg 'lib/libwdi.lib') -Force
Copy-Item (Join-Path $src 'COPYING-LGPL') (Join-Path $pkg 'COPYING-LGPL') -Force
Copy-Item (Join-Path $root 'patches/*.patch') (Join-Path $pkg 'patches') -Force
"$LibwdiRef" | Set-Content (Join-Path $pkg 'upstream-ref.txt') -Encoding ascii
# 上游源码/构建脚本的可获取地址（LGPLv3 合规材料之一）
"https://github.com/pbatard/libwdi/tree/$LibwdiRef" | Set-Content (Join-Path $pkg 'UPSTREAM-SOURCE.txt') -Encoding ascii

$libSha = (Get-FileHash (Join-Path $pkg 'lib/libwdi.lib') -Algorithm SHA256).Hash.ToLowerInvariant()
$info = [ordered]@{
    schema       = 1
    upstream     = 'libwdi'
    upstream_tag = 'v1.5.1'
    upstream_ref = $LibwdiRef
    platform     = 'x64'
    payload      = 'WinUSB only (OPT_M32/OPT_ARM/LIBUSB0_DIR/LIBUSBK_DIR disabled)'
    toolset      = (& msbuild -version | Select-Object -Last 1)
    runner_image = $env:ImageOS
    built_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    lib_sha256   = $libSha
}
$info | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $pkg 'BUILD-INFO.json') -Encoding utf8

# SHA256SUMS：对 pkg 内每个文件记录相对路径
$sums = Get-ChildItem $pkg -Recurse -File | ForEach-Object {
    $rel = $_.FullName.Substring($pkg.Length + 1) -replace '\', '/'
    '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $rel
}
$sums | Set-Content (Join-Path $pkg 'SHA256SUMS') -Encoding ascii

# ---- 6) 打 zip（版本化文件名）---------------------------------------------
$zip = Join-Path $OutDir 'libwdi-1.5.1-x64.zip'
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $pkg '*') -DestinationPath $zip -Force

# ---- 7) 给 CI 导出（保持 C2 契约）-----------------------------------------
if ($env:GITHUB_ENV) {
    "LIBWDI_LIB_DIR=$(Join-Path $pkg 'lib')" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
    "LIBWDI_LIB_SHA256=$libSha" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
}
if ($env:GITHUB_OUTPUT) {
    "lib_sha256=$libSha" | Out-File $env:GITHUB_OUTPUT -Append -Encoding utf8
    "zip=$zip" | Out-File $env:GITHUB_OUTPUT -Append -Encoding utf8
}

Write-Host "==> libwdi.lib  : $(Join-Path $pkg 'lib/libwdi.lib')"
Write-Host "==> sha256      : $libSha"
Write-Host "==> artifact zip: $zip"

if (-not $KeepWorkDir) {
    # 只清 msbuild 中间产物；保留 pkg/ 供 upload-artifact
    Write-Host "==> work dir kept at $WorkDir (msbuild intermediates)"
}
```

#### 3.3.6 `tools/verify-libwdi.ps1`（完整骨架）

```powershell
<#
.SYNOPSIS
    校验 libwdi 产物契约（存在、SHA256、导出符号、/MT、LGPL 文本）。
.NOTES
    草案。LIBCMT 字符串探测是启发式：实施时先用真实产物验证；
    最终以产品 exe 的 dumpbin /dependents 无 vcruntime/msvcp/ucrt 为准（见 7.2）。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LibDir,
    [string]$ExpectedSha256 = '',
    [string]$Dumpbin = ''
)
$ErrorActionPreference = 'Stop'

function Find-Dumpbin {
    if ($Dumpbin) { return $Dumpbin }
    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (Test-Path $vswhere) {
        $found = & $vswhere -latest -products * -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' 2>$null
        if ($found) { return @($found)[0] }
    }
    $cmd = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw 'dumpbin.exe not found; pass -Dumpbin'
}

$lib = Join-Path $LibDir 'libwdi.lib'
if (-not (Test-Path $lib)) { throw "missing $lib" }
$size = (Get-Item $lib).Length
if ($size -lt 1MB) { throw "libwdi.lib is suspiciously small: $size bytes" }

# 1) SHA256
$sha = (Get-FileHash $lib -Algorithm SHA256).Hash.ToLowerInvariant()
if ($ExpectedSha256 -and $sha -ne $ExpectedSha256.ToLowerInvariant()) {
    throw "libwdi.lib SHA256 mismatch: expected $ExpectedSha256, got $sha"
}

# 2) 导出符号（静态库 = COFF 符号）
$dumpbin = Find-Dumpbin
$symbols = & $dumpbin /nologo /symbols $lib
foreach ($name in @('wdi_create_list','wdi_destroy_list','wdi_prepare_driver',
                    'wdi_install_driver','wdi_strerror','wdi_set_log_level')) {
    if (-not ($symbols | Select-String -SimpleMatch $name)) {
        throw "libwdi.lib is missing symbol $name"
    }
}

# 3) /MT 启发式：静态 CRT 的 .drectve 含 /DEFAULTLIB:LIBCMT
$ascii = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($lib))
if ($ascii -notmatch 'LIBCMT') { throw 'libwdi.lib does not default-link LIBCMT (built with /MD?)' }
if ($ascii -match 'MSVCRT|LIBCMTD') { throw 'libwdi.lib references MSVCRT or the debug CRT' }

# 4) LGPL 文本
$copying = Join-Path (Split-Path $LibDir -Parent) 'COPYING-LGPL'
if (-not (Test-Path $copying)) { throw 'artifact is missing COPYING-LGPL' }
$text = Get-Content $copying -Raw
if ($text -notmatch 'GNU LESSER GENERAL PUBLIC LICENSE') { throw 'COPYING-LGPL is not LGPL text' }
if ($text -notmatch 'Version 3') { throw 'COPYING-LGPL is not LGPLv3' }

# 5) BUILD-INFO.json 自洽
$infoPath = Join-Path (Split-Path $LibDir -Parent) 'BUILD-INFO.json'
if (-not (Test-Path $infoPath)) { throw 'artifact is missing BUILD-INFO.json' }
$info = Get-Content $infoPath -Raw | ConvertFrom-Json
if ($info.lib_sha256 -ne $sha) { throw "BUILD-INFO.json lib_sha256 does not match $sha" }

Write-Host "libwdi.lib OK: sha256=$sha size=$size"
if ($env:GITHUB_OUTPUT) {
    "sha256=$sha" | Out-File $env:GITHUB_OUTPUT -Append -Encoding utf8
}
```

#### 3.3.7 `tools/apply-patches.ps1`（骨架）

```powershell
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Repo,
    [Parameter(Mandatory)][string]$PatchDir
)
$ErrorActionPreference = 'Stop'
foreach ($p in (Get-ChildItem $PatchDir -Filter '*.patch' | Sort-Object Name)) {
    & git -C $Repo apply --check --whitespace=nowarn $p.FullName
    if ($LASTEXITCODE -ne 0) { throw "patch does not apply: $($p.Name)" }
    & git -C $Repo apply --whitespace=nowarn $p.FullName
    if ($LASTEXITCODE -ne 0) { throw "patch failed: $($p.Name)" }
    Write-Host "applied $($p.Name)"
}
```

#### 3.3.8 `patches/` 清单

| patch | 目标文件 | 修改内容（等价于现状 `build.yml` 的正则） |
| --- | --- | --- |
| `0001-config-win32-x64-only.patch` | `msvc/config.h` | `#define OPT_M32`、`#define OPT_ARM` 注释掉（x64-only 载荷）；`LIBUSB0_DIR`、`LIBUSBK_DIR` 注释掉（只用 WinUSB）（`build.yml:128-133`） |
| `0002-libwdi-static-drop-project-references.patch` | `libwdi/.msvc/libwdi_static.vcxproj` | 删除全部 `<ProjectReference>…</ProjectReference>`，避免 `Release x64` 平台配置解析 `embedder/installer_x86` 触发 MSB8013（`build.yml:147-153`） |

补丁生成方式（一次性，实施时执行并提交结果）：

```powershell
git clone --branch v1.5.1 https://github.com/pbatard/libwdi.git scratch-libwdi
# 在 scratch-libwdi 里按上表手工修改（与旧 workflow 的 -replace 完全等价）
git -C scratch-libwdi diff > patches/0001-config-win32-x64-only.patch
# 工程引用删除单独生成 0002
```

> 实施注意：`.gitattributes` 要对 `patches/*` 设 `-text`，否则 Windows 的 EOL 转换会让 `git apply` 失败。补丁是否仍适用于上游由 `apply-patches.ps1` 的 `--check` 保证（上游一旦变化会显式失败，而不是静默漏改——修 D2）。

#### 3.3.9 `.github/workflows/libwdi-static.yml`（可复用 + 自测 + 发布）

```yaml
name: libwdi-static

on:
  workflow_call:
    inputs:
      libwdi_ref:
        description: "libwdi upstream commit SHA (v1.5.1)"
        type: string
        required: false
        default: "<LIBWDI_V151_SHA>"   # 与 upstream/libwdi.ref 保持一致
    outputs:
      libwdi_lib_sha256:
        description: "SHA256 of lib/libwdi.lib inside the artifact"
        value: ${{ jobs.build.outputs.libwdi_lib_sha256 }}
  push:
    branches: [libwdi-static]
    tags: ["libwdi-*"]
  workflow_dispatch:

permissions:
  contents: read

env:
  WDK_LINK_ID: "253170"
  WDK_MSI_SHA256: "<msi-sha256>"   # 首次实施时记录；镜像/微软更新后由 PR 修改

jobs:
  build:
    name: Build & verify static libwdi (x64, WinUSB payload)
    runs-on: windows-2022          # 固定镜像，不用 windows-latest（规避工具集漂移，见 6.2）
    timeout-minutes: 30
    outputs:
      libwdi_lib_sha256: ${{ steps.verify.outputs.sha256 }}
    steps:
      - name: Checkout libwdi-static content (not the caller commit)
        uses: actions/checkout@v4
        with:
          # workflow_call 时 github.sha 是调用方提交；必须取被调用工作流的提交。
          ref: ${{ github.job_workflow_sha || github.sha }}

      - uses: microsoft/setup-msbuild@v2

      - name: Cache WDK redist MSI
        uses: actions/cache@v4
        with:
          path: .cache/wdk-redist.msi
          key: wdk-redist-${{ env.WDK_LINK_ID }}-${{ env.WDK_MSI_SHA256 }}

      - name: Build libwdi.lib
        shell: pwsh
        env:
          WDK_MSI_SHA256: ${{ env.WDK_MSI_SHA256 }}
        run: ./tools/build-libwdi.ps1 -LibwdiRef "${{ inputs.libwdi_ref }}" -OutDir dist

      - name: Verify artifact contract
        id: verify
        shell: pwsh
        run: ./tools/verify-libwdi.ps1 -LibDir dist/lib

      - name: Upload artifact (same-run consumers + release job)
        uses: actions/upload-artifact@v4
        with:
          name: libwdi-1.5.1-x64
          path: dist/
          if-no-files-found: error

  release:
    name: Publish release asset
    if: startsWith(github.ref, 'refs/tags/libwdi-')
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: libwdi-1.5.1-x64
          path: dist
      - name: Create release with pinned assets
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          tag="${GITHUB_REF_NAME}"
          gh release create "$tag" \
            dist/libwdi-1.5.1-x64.zip dist/SHA256SUMS dist/BUILD-INFO.json \
            --repo "$GITHUB_REPOSITORY" --title "$tag" \
            --notes-file dist/RELEASE-NOTES.md
```

#### 3.3.10 `.github/workflows/consume-smoke.yml`（消费者视角自测）

```yaml
name: consume-smoke

on:
  workflow_dispatch:
  push:
    branches: [libwdi-static]

permissions:
  contents: read

jobs:
  consume-asset-and-crate:
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.job_workflow_sha || github.sha }}
      - uses: dtolnay/rust-toolchain@stable

      - name: Download the pinned release asset and verify it
        shell: pwsh
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          New-Item -ItemType Directory -Force .libwdi | Out-Null
          gh release download libwdi-1.5.1-x64-r1 --repo lazybri/aurokku-rs `
            --pattern 'libwdi-1.5.1-x64.zip' --dir .libwdi
          $hash = (Get-FileHash .libwdi/libwdi-1.5.1-x64.zip -Algorithm SHA256).Hash.ToLowerInvariant()
          if ($hash -ne '<asset-sha256>') { throw "asset hash mismatch: $hash" }
          Expand-Archive .libwdi/libwdi-1.5.1-x64.zip -DestinationPath .libwdi/pkg

      - name: Link smoke test (cargo test --no-run)
        shell: pwsh
        env:
          LIBWDI_LIB_DIR: ${{ github.workspace }}/.libwdi/pkg/lib
        run: |
          cargo test --no-run --manifest-path crates/libwdi-sys/Cargo.toml

      - name: External consumer resolves the crate through git tag
        shell: pwsh
        run: |
          New-Item -ItemType Directory -Force ../consume | Out-Null
          Push-Location ../consume
          @'
          [package]
          name = "consume-smoke"
          version = "0.0.0"
          edition = "2021"

          [lib]
          path = "lib.rs"

          [target.'cfg(windows)'.dependencies]
          libwdi-sys = { git = "https://github.com/lazybri/aurokku-rs", tag = "libwdi-1.5.1-x64-r1" }
          '@ | Set-Content Cargo.toml -Encoding utf8
          "pub fn f() { let _ = libwdi_sys::WDI_SUCCESS; }" | Set-Content lib.rs -Encoding utf8
          cargo metadata --format-version 1 | Out-Null
          Pop-Location
```

### 3.4 为什么分支根 manifest 必须是"纯 workspace"

产品仓库会这样引用：

```toml
[target.'cfg(windows)'.dependencies]
libwdi-sys = { git = "https://github.com/lazybri/aurokku-rs", tag = "libwdi-1.5.1-x64-r1", optional = true }
```

Cargo 解析 git 依赖的流程是：先取该 rev 的**仓库根** `Cargo.toml`，再在其中找 `name = "libwdi-sys"` 的包。

- 若根是 **virtual workspace**（只有 `[workspace]`，没有 `[package]`），Cargo 会按 `members` 枚举成员，从而找到 `crates/libwdi-sys`；
- 若根同时有 `[package]` 且名字不是 `libwdi-sys`，默认解析只命中根包，容易报
  `no matching package named 'libwdi-sys' found`（即使 `package = "libwdi-sys"` 也依赖具体 Cargo 版本的 workspace 扫描行为）。

因此本设计选择**根为纯 workspace、唯一成员是 `crates/libwdi-sys`**，并额外用 `consume-smoke.yml` 的"外部空 crate + `cargo metadata`"把这个行为固化成 CI 门禁。若未来要在分支里加第二个 crate（例如 `libwdi-tool`），只需加进 `members`，产品侧引用方式不变。

---

## 4. 共享引用机制对比（A/B/C/D）

### 4.1 方案 A：同仓库跨分支可复用工作流（`workflow_call`）

**做法**：`libwdi-static` 分支提供 `.github/workflows/libwdi-static.yml`（`on: workflow_call`），main/qt6 的 `build.yml` 增加一个 `uses` job，产物通过**同一次 run 内**的 `actions/upload-artifact` + `actions/download-artifact` 传给后续构建 job。

```yaml
# main/.github/workflows/build.yml（草案改动）
jobs:
  libwdi:
    name: Build static libwdi
    permissions:
      contents: read
    # 关键：@ref 必须是字面量，不能用 ${{ }} 表达式；优先用 tag/SHA 而不是分支名。
    uses: lazybri/aurokku-rs/.github/workflows/libwdi-static.yml@libwdi-1.5.1-x64-r1
    with:
      libwdi_ref: "<LIBWDI_V151_SHA>"

  build:
    needs: [test, libwdi]
    runs-on: windows-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: libwdi-1.5.1-x64
          path: .libwdi
      - name: Expose LIBWDI_LIB_DIR
        shell: pwsh
        run: |
          "LIBWDI_LIB_DIR=${{ github.workspace }}/.libwdi/lib" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
      - run: ./.github/scripts/build-windows.ps1 -Target x86_64-pc-windows-msvc
```

**必须注意的坑**：

1. **checkout 的 ref**：被调用工作流里 `github.sha`/`github.ref` 是**调用方**的；必须用 `github.job_workflow_sha` 才能 checkout 到 `libwdi-static` 分支内容（`3.3.9` 已写）。
2. **产物只能在同一次 run 内传**：`actions/download-artifact@v4` 默认只下载当前 run 的 artifact；跨 run 必须改成 `run-id` + `github-token`，而且 v4 **不支持跨 run 下载**（v3 才可以）——所以 A 方案必须"一次 run 内先构建 libwdi 再构建产品"，即多一个 job，不能复用历史产物。
3. **`uses` 里不能写表达式**：版本 pin 只能写死 `@ref`（tag 或 SHA），升级 libwdi 需要改产品仓库的 workflow（一次 PR，可接受）。
4. **权限**：`libwdi-static.yml` 顶层 `permissions: contents: read` 足够（只需要 clone 上游 + 上传 artifact）；调用方不能给被调用方"升级"权限。若分支是私有的，`checkout` 上游和本分支都需要 token（同仓库 `GITHUB_TOKEN` 即可）。
5. **public 限制**：跨**仓库**复用要求被调用仓库 public（或同 org 且允许）；**同仓库跨分支**没有这个限制，但 `uses: ./.github/...` 的"本地路径"形式只能引用**同一提交**，跨分支必须写 `owner/repo/.github/workflows/x.yml@ref`。
6. **缓存作用域**：`actions/cache` 按分支作用域隔离；在 `libwdi-static` 分支上缓存的 WDK MSI，main 分支的 run 读不到；但反过来，因为被调用 job 属于**调用方 run**，缓存会落在调用方分支名下——即 main 首次仍要下一次 31 MB。
7. **artifact 名称冲突**：artifact 在 run 级全局唯一，矩阵/多产品时给名字加后缀。
8. **可复现性取决于 ref**：`@libwdi-static` 分支名会漂移；`@<sha>` 最稳但升级要改产品仓库；折中用 `@<release tag>`。

### 4.2 方案 B：预编译产物（Release asset + SHA256 pin）——推荐

**做法**：`libwdi-static` 分支在 tag `libwdi-1.5.1-x64-r1` 上发布 release asset（`3.3.9` 的 `release` job）。main/qt6 的 Windows job 直接下载、校验、解压，设置 `LIBWDI_LIB_DIR`；**不需要 MSBuild/WDK/git clone**。

**main 侧 YAML/PowerShell 片段**：

```yaml
- name: Fetch pinned static libwdi
  shell: pwsh
  env:
    GH_TOKEN: ${{ github.token }}
    LIBWDI_ASSET_TAG: libwdi-1.5.1-x64-r1
    LIBWDI_ASSET_SHA256: "<asset-sha256>"   # 信任锚：main 仓库内 review 过的常量
  run: |
    New-Item -ItemType Directory -Force .libwdi | Out-Null
    gh release download $env:LIBWDI_ASSET_TAG --repo lazybri/aurokku-rs `
      --pattern 'libwdi-1.5.1-x64.zip' --dir .libwdi
    $zip = Join-Path '.libwdi' 'libwdi-1.5.1-x64.zip'
    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $env:LIBWDI_ASSET_SHA256) {
        throw "libwdi asset SHA256 mismatch: $actual"
    }
    Expand-Archive $zip -DestinationPath .libwdi/pkg -Force
    # C2 契约不变：产品脚本/build.rs 只认这个变量
    "LIBWDI_LIB_DIR=${{ github.workspace }}/.libwdi/pkg/lib" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
```

**优点**：

- 产品 CI 不再依赖 WDK/MSVC 工具集、上游仓库、微软 redirect；Windows job 少 4 次 msbuild，构建时间显著下降（**需实测**，见 `6.3）。
- 产物**不可变 + 可审计**：asset 内带 `BUILD-INFO.json`/`SHA256SUMS`/`patches/`；main 仓库里的 `LIBWDI_ASSET_SHA256` 常量是真正的信任锚（release 被替换 → hash 失败）。
- PR 也能低成本跑 Windows 链接验证（只下载 + `cargo test --no-run`），修 D5。
- qt6 复用零成本：同一个下载 step。

**坑**：

- `gh release download` 需要 `GH_TOKEN`（同仓库 `contents: read` 够；public 仓库也可以匿名 `Invoke-WebRequest` 到 `https://github.com/<owner>/<repo>/releases/download/<tag>/<asset>`，但 `gh` 更简单且带重试）。
- **不要对"重新构建出的 libwdi.lib"做 hash pin**：MSVC 默认在 COFF 里写入时间戳/PDB 路径，同一源码两次构建未必字节一致（除非开 `/Brepro`）。正确做法是 pin **下载到的 zip asset** 的 SHA256（不可变发布物），并接受"构建可重复但字节非确定"。
- 供应链：能改 release asset 的人 = 能改产品构建的人；因此 tag 必须保护、`LIBWDI_ASSET_SHA256` 变更必须走 main 的 PR。
- 首次引入需要先在 `libwdi-static` 分支发一次 release（bootstrap 顺序见 `4.6）。

### 4.3 方案 C：cargo git 依赖（绑定 crate 走新分支）

**做法**（Cut2）：`libwdi-sys` 用 git tag 引用。

```toml
# main / qt6 的 Cargo.toml
[target.'cfg(windows)'.dependencies]
libwdi-sys = { git = "https://github.com/lazybri/aurokku-rs", tag = "libwdi-1.5.1-x64-r1", optional = true }

[features]
static-libwdi = ["dep:libwdi-sys"]
```

**Cargo.lock 会记录**（必须提交，否则 `--locked` 失败）：

```toml
[[package]]
name = "libwdi-sys"
version = "0.1.0"
source = "git+https://github.com/lazybri/aurokku-rs?tag=libwdi-1.5.1-x64-r1#<40-hex-rev>"
```

**优点**：绑定/链接指令/版本/许可单一事实源；qt6 只需改 manifest；`build.rs` 可以自己做产物校验；产品 CI 不需要知道分支布局。

**坑**：

- 根 manifest 必须是纯 workspace（`3.4`），否则解析不到 crate。
- 必须改 `Cargo.lock` 并提交；main/qt6 的构建脚本都用 `--locked`（`build-windows.ps1:136`、qt6 `build-windows.ps1:63`），lock 不同步会直接失败。
- 每次清缓存构建要访问 GitHub 拉该分支（首次约几百 ms–数秒，取决于网络）；`Swatinem/rust-cache` 默认缓存 `~/.cargo/git/db`，可缓解。
- **私有仓库**：需要 `CARGO_NET_GIT_FETCH_WITH_CLI=true` + 凭据（`git config --global url."https://x-access-token:$TOKEN@github.com/".insteadOf "https://github.com/"`），或在 CI 里 `actions/checkout@v4` + `GITHUB_TOKEN` 注入。public 仓库无此问题。
- **离线构建**：必须 `cargo vendor`（会把 git 依赖 vendor 进目录）或用本地 `[patch]` 指向 path；纯 `--offline` 第一次必然失败。
- tag 可变：移动 tag 会让未更新 lock 的机器重新解析到不同 rev（`--locked` 时可检测失败）。建议 `rev = "<sha>"` 最稳，`tag` 次之，`branch` 最差。
- 审查链路变长：改绑定要走 `libwdi-static` 分支的 PR + 产品仓库的 lock 更新 PR。

### 4.4 方案 D：git submodule / git subtree

**做法**：

```powershell
git submodule add -b libwdi-static https://github.com/lazybri/aurokku-rs.git third_party/libwdi-static
# CI:
- uses: actions/checkout@v4
  with: { submodules: recursive }
```

**问题**：

- submodule 的 ref 是**父仓库 tree 里的 gitlink**（固定 SHA），`.gitmodules` 的 `branch` 只影响 `git submodule update --remote`，不改变默认 checkout 的版本——"跟随分支"是错觉。
- main 和 qt6 各自记录一个 gitlink，两个分支合并/分别升级时会在 `third_party/libwdi-static` 上冲突（与附录 A.3 的冲突性质相同）。
- path 依赖落在产品 workspace 根目录下时，会被 Cargo **自动纳入 workspace**（路径依赖即成员），于是产品 workspace 的 `cargo fmt --all`/`clippy --workspace` 会扫描子模块内容；需要 `exclude` 管理，容易踩坑。
- submodule 目录里没有 `libwdi.lib`，**仍然要配合 B 或现场构建**；单独用 D 并不能解耦二进制。
- 离线/镜像/浅克隆场景容易漏 `--recursive`。

结论：**不推荐**（除了"只想共享脚本、不在意二进制"的小团队场景）。

### 4.5 对比表

| 维度 | A workflow_call（源码构建） | B 预编译 asset（推荐） | C cargo git 依赖 | D submodule |
| --- | --- | --- | --- | --- |
| 产品 CI 构建时间 | 每次 +WDK 下载/解包 + clone + 4×msbuild（约数分钟，需实测） | 每次 +下载 ~几 MB + 解压 + hash（秒级） | 仅 crate 编译；产物另需 B 或现场构建 | 同 A（除非配合 B） |
| 可复现性 | 好（源码+固定 ref；但受上游 tag/镜像影响） | 中（发布物固定；重建字节不保证） | 好（lock 固定 rev） | 中 |
| 缓存收益 | WDK MSI 可按分支缓存；作用域受限 | asset 可缓存；跨分支要各自预热 | `~/.cargo/git/db` 可缓存 | submodule 有 git 缓存 |
| 维护成本 | 中（每次升级改产品 workflow 的 `@ref`） | 低（只改 tag + hash 常量） | 中（lock 更新 + 分支 PR） | 高（双分支 gitlink + workspace） |
| 审查/审计 | 好（源码可读） | 好（asset 内带 BUILD-INFO/patches/源地址 + hash pin） | 好（crate 源码可读） | 一般 |
| 主要失败模式 | job 依赖/artifact 名/ref 漂移；上游/微软网络 | asset 被替换（hash 抓）、tag 被删 | lock 漂移、私仓凭据、离线失败 | 漏 `--recursive`、gitlink 冲突 |
| qt6 复用 | 只需多一个 `needs`+下载 | 零成本 | 只需加依赖 | 需要子模块 + 构建 |
| 离线构建 | 差（必须联网 clone+MSI） | 中（可缓存 zip） | 差（首次联网） | 中 |
| 供应链风险 | 低（每次从源码构建） | 中（依赖 release 资产，需 hash pin + tag 保护） | 低-中（依赖分支内容） | 中 |

### 4.6 推荐方案、回退与迁移步骤

**主方案（目标态）：B + C**

- B 负责**二进制**（`libwdi.lib` 及许可/补丁/元数据）：产品 CI 只下载 + 校验 + 设置 `LIBWDI_LIB_DIR`。
- C 负责**代码**（`libwdi-sys` 绑定与链接指令）：第二阶段启用，让 main/qt6 共享同一份绑定。
- 先做 Cut1（B，crate 暂留），再做 Cut2（B+C）。

**回退方案：A**

- 触发条件：release asset 不可用（私仓策略、组织禁止二进制分发、供应链要求每次源码构建）、或 `gh release download` 在目标环境受限。
- 回退动作：把产品 `uses` 指向 `libwdi-static.yml@<pin>`，其余契约（C1/C2）不变——这也是为什么 `3.3.9` 的工作流同时支持 `workflow_call` 与 tag 发布。

**不采用：D**。

**迁移步骤（建议按此顺序，每步可独立回滚）**

1. **建分支**：从 main 最新提交开 orphan 分支 `libwdi-static`；提交 `3.2` 目录骨架（脚本 + patches + crate 占位 + 文档 + 工作流）。
2. **生成并验证 patches**：在 v1.5.1 上生成 `0001/0002`，确认 `apply-patches.ps1` 通过；记录 `upstream/libwdi.ref` 的 40-hex SHA。
3. **自测**：`workflow_dispatch` 跑 `libwdi-static.yml`，确认 `verify-libwdi.ps1` 全绿；把 `libwdi.lib` 的 SHA256、asset SHA256、WDK MSI SHA256 写进 `BUILD-INFO.json` 与本分支 README。
4. **发第一个 release**：打 tag `libwdi-1.5.1-x64-r1`，确认 `release` job 产出 asset + `SHA256SUMS`，并跑 `consume-smoke.yml`（B 下载 + C git 解析）。
5. **main 切 Cut1**：删 `build.yml:108-157` 的 WDK/libwdi 步骤，改为 `4.2` 的下载+校验 step；`build-windows.ps1` 只改 `:163-171` 的许可来源（`<artifact>/COPYING-LGPL`）；更新 `docs/windows-driver.md` 与 README 链接。
6. **main 验证**：Windows job 绿；`dumpbin /dependents` 无 `libwdi.dll`/`libusb-1.0.dll`/运行库；zip 含 `LICENSE-LGPL-3.0.txt`；对比改动前后的 zip 内容与 exe 导入表。
7. **qt6 接入**（见 `5）：先 Cut1（下载 + feature + CMake 链接 libwdi.lib + 打包许可），再判断是否合并到 Cut2。
8. **Cut2**：把 crate 迁到分支 → 产品 `Cargo.toml` 改 git 依赖 → `cargo update -p libwdi-sys` 提交 `Cargo.lock` → 两分支 `--locked` 构建验证。Cut2 完成后，产品仓库删除 `crates/auroku-driver-sys` 与 workspace 成员。
9. **门禁补强**：产品仓库加一个 `workflow_dispatch`/`pull_request` 的 Windows link-smoke job（下载 asset + `cargo test --no-run`），修 D5。

---

## 5. 与 qt6 的接入

### 5.1 qt6 现状证据：现在完全没有 libwdi / 驱动安装

| 断言 | 证据 |
| --- | --- |
| 没有 `src/driver.rs` | `git -C <qt6> ls-tree -r --name-only HEAD` 只列出 `src/lib.rs`、`src/main.rs`（无 `driver.rs`） |
| 没有 driver-sys crate | 同上，`crates/` 下只有 `crates/auroku-ffi/{Cargo.toml,src/lib.rs}` |
| 没有 feature/依赖 | `Cargo.toml:1-30` 无 `static-libwdi`、无 `[target.'cfg(windows)'.dependencies]`（只有一个 linux 的 `rustix`） |
| `src/lib.rs` 无驱动安装 | main `src/lib.rs:24` 是 `pub mod driver;`，qt6 对应位置没有任何模块声明；`AuRokkuError` 也没有 main 的 `Driver(String)` 变体（qt6 `src/lib.rs:39-47` vs main `src/lib.rs:42-52`） |
| 构建脚本无 libwdi | `build-windows.ps1:1-257` 无 `LIBWDI`/`libwdi` 字样 |
| CI 无 libwdi | `.github/workflows/build.yml:1-215` 无 libwdi 步骤（只有 MSVC 环境 `:99-101` 与 Qt 安装 `:103-108`） |
| 全仓库 grep 为 0 | 关键词 `libwdi`、`WinUSB`、`wdi`、`install-driver` 在 qt6 全仓库 → 0 命中（main 同 grep 有 80+ 命中） |

qt6 的 Windows 构建路径（改造基线）：`build-windows.ps1:62-99` 构建 CLI（`--bin auroku`）；`gui/CMakeLists.txt:59-66` 用 `cargo build -p auroku-ffi --release` 构建 Rust 后端为 staticlib；`:139-140` 把 `auroku_ffi.lib` 链进 Qt exe；`:141-144` 补系统库；`build-windows.ps1:241-249` 打包 `dist/auroku.exe` + `dist/auroku-gui-windows-x86_64/`。

### 5.2 接入改动点（复用同一个 libwdi 产物）

| # | 文件 | 改动 | 为什么 |
| --- | --- | --- | --- |
| Q1 | qt6 `Cargo.toml` | 增加 `[features] static-libwdi = ["dep:libwdi-sys"]`；增加 `[target.'cfg(windows)'.dependencies] libwdi-sys = { git = ..., tag = "libwdi-1.5.1-x64-r1", optional = true }` | 复用 C3 契约；feature 名与 main 对齐（C4）。`[workspace]` 已是显式 `members = [".", "crates/auroku-ffi"]`（`:1-3`），git 依赖解析无需额外改动 |
| Q2 | qt6 `src/driver.rs`（新增）+ `src/lib.rs:24` | 从 main `src/driver.rs:1-719` 移植；`src/lib.rs` 加 `pub mod driver;` | 产品语义可复用；main 的 `install/restore` 返回 `Result<_, String>`，不依赖 main 的 `AuRokkuError::Driver` 变体，移植成本低 |
| Q3 | qt6 `crates/auroku-ffi/Cargo.toml` | 增加 `[features] static-libwdi = ["auroku/static-libwdi"]` | 让 CMake 用 `-p auroku-ffi --features static-libwdi` 打开后端 |
| Q4 | qt6 `crates/auroku-ffi/src/lib.rs` | 新增 `#[no_mangle] pub extern "C" fn auroku_install_driver(...)` / `auroku_restore_driver(...)` / `auroku_driver_pending()`（JSON envelope，同现有 `:345-444` 风格） | C++ 通过 JSON 调用；C++ 不链接 libwdi |
| Q5 | qt6 `gui/src/core/FfiApi.h`、`RustBridge.*`、相关页面（如 `OverviewPage`/`SessionPage`） | 声明并调用 Q4 的 FFI；按钮/菜单 + 错误展示 | GUI 功能入口（产品决定是否本阶段做） |
| Q6 | qt6 `gui/CMakeLists.txt` | ① `option(AUROKU_STATIC_LIBWDI ...)`；② `cargo build ... -p auroku-ffi ${AUROKU_CARGO_FLAGS} --features static-libwdi`（`:59-66`）；③ **最终链接追加 `libwdi.lib` + 缺失的系统库**（`:141-144` 已有 `ws2_32 userenv bcrypt ntdll advapi32 ole32 setupapi cfgmgr32 dbghelp user32 hid`，还需 `crypt32 wintrust version shlwapi msi shell32 oleaut32 uuid comdlg32 winspool comctl32 gdi32`） | **关键**：`staticlib` 不打包 native 库；CMake 必须补 `libwdi.lib` 与 19 个系统库（`1.3` 列表减去已列出的） |
| Q7 | qt6 `build-windows.ps1` | ① 检测 `$env:LIBWDI_LIB_DIR` → CLI 构建加 `--features static-libwdi`（对齐 main `:114-121`）；② 给 CMake 传 `-DAUROKU_STATIC_LIBWDI=ON`（`:151-152`）；③ 打包时把 `<artifact>/COPYING-LGPL` 复制为 `LICENSE-LGPL-3.0.txt` 并加入 zip（`:244-249`） | 复用 C2；合规 |
| Q8 | qt6 `.github/workflows/build.yml` | Windows job 加 `4.2` 的下载+校验 step（放在 `:116` 之前） | 拿到产物；无需 WDK/MSBuild |
| Q9 | qt6 `build-windows.ps1` 的校验段 | 把 `:84-99` 的 `objdump` 非系统 DLL 检查扩展到 **GUI exe**，并显式禁止 `libwdi.dll`/`libusb-1.0.dll` | 验收"PE 导入表无 libwdi/wdi 动态依赖" |

CMake 关键片段（草案）：

```cmake
option(AUROKU_STATIC_LIBWDI "Link the in-process libwdi WinUSB installer" OFF)

if(WIN32 AND AUROKU_STATIC_LIBWDI)
    set(AUROKU_FFI_FEATURES "--features" "static-libwdi")
    set(AUROKU_LIBWDI_LIB "$ENV{LIBWDI_LIB_DIR}/libwdi.lib")
    if(NOT EXISTS "${AUROKU_LIBWDI_LIB}")
        message(FATAL_ERROR "AUROKU_STATIC_LIBWDI=ON but LIBWDI_LIB_DIR/libwdi.lib not found")
    endif()
else()
    set(AUROKU_FFI_FEATURES "")
    set(AUROKU_LIBWDI_LIB "")
endif()

add_custom_target(auroku_ffi_build ALL
    COMMAND "${CMAKE_COMMAND}" -E env ${AUROKU_FFI_ENV}
            "${CARGO_EXECUTABLE}" build --locked --manifest-path "${AUROKU_ROOT}/Cargo.toml"
            -p auroku-ffi ${AUROKU_CARGO_FLAGS} ${AUROKU_FFI_FEATURES}
    WORKING_DIRECTORY "${AUROKU_ROOT}"
    BYPRODUCTS "${FFI_LIBRARY}"
    COMMENT "Building auroku-ffi (Rust C ABI, ${AUROKU_PROFILE_DIR})"
    VERBATIM)

if(WIN32 AND AUROKU_STATIC_LIBWDI)
    # Rust staticlib 不携带 native 依赖：libwdi.lib 必须由最终链接提供。
    target_link_libraries(auroku-qt PRIVATE "${AUROKU_LIBWDI_LIB}")
    target_link_libraries(auroku-qt PRIVATE
        crypt32 wintrust version shlwapi msi shell32 oleaut32 uuid
        comdlg32 winspool comctl32 gdi32)
endif()
```

> `build-windows.ps1` 必须在调用 CMake 前设置 `$env:LIBWDI_LIB_DIR`（CMake `$ENV{LIBWDI_LIB_DIR}` 继承进程环境）；推荐传参 `-DAUROKU_STATIC_LIBWDI=ON` 而不是在 CMake 里读环境自动开关（显式、可复现）。

### 5.3 "C++ 侧不需要直接链接 libwdi"的准确表述

- **源码层**：C++ 不 `#include` libwdi 头文件、不调用 `wdi_*`，只通过 `auroku-ffi` 的 C ABI（JSON envelope）间接触发。✅
- **链接层**：**不能**说"C++ 不需要 libwdi.lib"。Rust `staticlib` 只包含 Rust 目标文件；`libwdi.lib` 和 19 个系统导入库必须出现在**最终** `link.exe` 命令行上（由 CMake 传入即可，C++ 源码无感）。这是 qt6 与 main 最大的结构差异，务必在实现时验证：先不加 `libwdi.lib` 试链，应看到 `LNK2019: unresolved external symbol wdi_install_driver`；加上后消失。

### 5.4 qt6 验证命令

```powershell
# 0) 准备产物环境
$env:LIBWDI_LIB_DIR = (Resolve-Path .libwdi/pkg/lib).Path

# 1) 完整构建（CLI + Qt GUI）
./build-windows.ps1 -Target x86_64-pc-windows-msvc

# 2) CLI：只允许 Windows 系统 DLL
objdump -p dist/auroku.exe | Select-String 'DLL Name:'
# 期望：无 libwdi.dll / libusb-1.0.dll / vcruntime* / msvcp* / ucrtbase / api-ms-win-crt-*

# 3) GUI：同上（重点，qt6 之前从未检查过 GUI）
objdump -p dist/auroku-gui-windows-x86_64/auroku-gui.exe | Select-String 'DLL Name:'

# 4) dumpbin 交叉确认（MSVC 工具链）
dumpbin /nologo /dependents dist/auroku-gui-windows-x86_64/auroku-gui.exe

# 5) 静态库里的符号确实是"未定义、待最终链接解析"的（证明没把 libwdi 打进 .lib）
dumpbin /symbols target/release/auroku_ffi.lib | Select-String 'wdi_install_driver'
# 期望：UNDEF external（CMake 已用 libwdi.lib 解析）

# 6) 许可与冒烟测试
Get-ChildItem dist/auroku-windows-x86_64.zip
Expand-Archive dist/auroku-windows-x86_64.zip -DestinationPath .check -Force
Get-ChildItem .check -Recurse | Where-Object Name -eq 'LICENSE-LGPL-3.0.txt'
$env:QT_QPA_PLATFORM = 'offscreen'
Start-Process dist/auroku-gui-windows-x86_64/auroku-gui.exe -ArgumentList '--smoke-test' -Wait -PassThru
```

---

## 6. 风险与合规

### 6.1 LGPLv3 静态链接义务（最高优先级）

- libwdi 是 **LGPLv3**（`docs/windows-driver.md:193-195` 已声明）。静态链接发布二进制时，LGPLv3 `4 要求让接收者能够**重新链接**到修改后的库版本。仓库当前的做法（公开源码 + 公开构建脚本 + 打包 `COPYING-LGPL`）在"用户可自行重建"的意义上成立，但有两个薄弱点：
  1. `COPYING-LGPL` 是**条件打包**（`build-windows.ps1:164-171`，文件不存在就静默跳过）——本地/异构构建可能漏；
  2. 当前的构建过程**修改了 libwdi 源码**（`config.h`）与工程文件（vcxproj），但这些修改只存在于 workflow 的 `-replace` 里（`build.yml:126-134`、`:150-153`），第三方要重建必须精确复刻 workflow 行为，而不是应用一组公开 patch。
- 解耦后的改进：产物内固定带 `COPYING-LGPL` + `patches/` + `upstream-ref.txt` + `UPSTREAM-SOURCE.txt`（`3.3.5`）；分支公开可访问；产品 zip 无条件带 `LICENSE-LGPL-3.0.txt`；`docs/LGPL-COMPLIANCE.md` 写清"如何替换 libwdi.lib 并重链接"。
- 仍需注意：如果未来把 `auroku-ffi`/GUI 做成**闭源**分发，则必须额外提供应用侧目标文件或改用动态链接 libwdi；当前全仓库 MIT + 公开源码，满足度较高。
- 建议在 `verify-libwdi.ps1` 里把 `LICENSES/LGPL-3.0.txt` 与上游 clone 的 `COPYING-LGPL` 做 hash 比对，防止上游换版本而产品仍声明 LGPLv3。

### 6.2 风险表

| 风险 | 触发条件 | 影响 | 缓解 |
| --- | --- | --- | --- |
| 上游 tag 被移动/删除 | 上游维护者 re-tag `v1.5.1` | 产物静默变化、不可复现 | 固定完整 SHA（`upstream/libwdi.ref`）+ `git checkout` 后 `rev-parse` 断言（`3.3.5`） |
| windows-latest 工具集漂移 | GitHub 升级镜像、VS 组件变化 | `libwdi_static.vcxproj` 的 `PlatformToolset` 不可用（MSB8020）、`dumpbin` 路径变化 | 分支 job 固定 `windows-2022`；必要时加 `0003-toolset.patch` 覆盖 `PlatformToolset`；把 `toolset` 写进 `BUILD-INFO.json` |
| WDK redist 下载失败/变更 | 微软 redirect 或 MSI 下线 | CI 直接失败 | 缓存 MSI + 校验 SHA256；在 `BUILD-INFO.json` 记录 `WDK_LINK_ID`；备用镜像策略需单独评审（WDK 许可限制再分发） |
| 预编译资产被替换 | 有 release 写权限的人/被盗 token | 供应链攻击 | tag 保护 + asset SHA256 pin 在 main 仓库（PR 审查）+ `gh attestation`（可选，未来） |
| `--locked` 失败 | Cut2 后 `Cargo.lock` 未更新 | 产品构建失败 | 迁移步骤明确要求 `cargo update -p libwdi-sys` 并提交；PR 模板加检查项 |
| 网络受限/离线 | 内网 runner、代理 | B/C/A 都失败 | B 可缓存 zip；C 支持 `cargo vendor`；A 不适用；文档写明离线流程 |
| 多产品分支版本漂移 | main 用 r1、qt6 用 r2 | 行为不一致、许可文件差异 | 统一 `libwdi-static` 的 `UPGRADE.md`：升级 = 分支发新 tag → 两个产品分支各一个 PR 改 tag/hash → 记录兼容矩阵 |
| 分支保护缺失 | orphan 分支无 review | 补丁/脚本被随意改 | 管理员开启分支保护 + tag 保护；`CODEOWNERS`（若仓库启用） |
| PR 门禁缺失（D5） | 目前 PR 不构建 Windows | 静态链接改动合并后才炸 | 加 `link-smoke` job（B 方案成本极低） |
| 缓存投毒/陈旧 | `actions/cache` 命中旧 MSI/旧 lib | 产物与声明不符 | cache key 含 MSI SHA256 与 tag；使用前重新校验 `libwdi.lib` hash（`verify-libwdi.ps1` 已做） |
| 构建时间不可控 | 每次 CI 重复 msbuild | PR 反馈慢、费用 | B 方案把构建收敛到 `libwdi-static` 分支（版本升级时一次），产品 CI 秒级 |

### 6.3 构建时间与缓存（测量计划，不虚构数值）

本文**没有**实测数据（未运行 CI）。实施时按下表记录，填回本文与分支 README：

| 场景 | 测量方法 | 记录字段 |
| --- | --- | --- |
| 现状 main Windows job（含 libwdi） | GitHub Actions run 的 job 时长 | 总时长 / `Build static libwdi` step 时长 |
| B 方案 main Windows job | 同一 job 改造前后对比 | 总时长 / `Fetch pinned static libwdi` step 时长 |
| A 方案（每次源码构建） | 同上 | 同上 |
| 缓存命中/未命中 | `actions/cache` 日志 | 命中率 |

缓存建议：

- WDK MSI：`key: wdk-redist-${WDK_LINK_ID}-${WDK_MSI_SHA256}`，`path: .cache/wdk-redist.msi`（`3.3.9` 已写）。
- libwdi 产物：产品侧不缓存（直接下载，秒级）；若要缓存 zip，key 含 `LIBWDI_ASSET_TAG` + `LIBWDI_ASSET_SHA256`。
- Rust：继续用 `Swatinem/rust-cache@v2`（`build.yml:106`、qt6 `build.yml:114`），它缓存 `~/.cargo/git/db`，缓解 C 方案首次拉取。

### 6.4 分支保护与权限（管理员操作清单）

1. 保护 `libwdi-static`：require PR + 1 approval、dismiss stale reviews、禁止 force-push（保护 Release tag 指向的内容）。
2. 保护 `libwdi-*` tags：禁止删除/移动（GitHub tag protection rules）。
3. 产品仓库：`LIBWDI_ASSET_SHA256` 常量变更必须走 PR（CODEOWNERS 指向 CI owner）。
4. 若仓库为 private：复用 `workflow_call` 与 `gh release download` 都需要 `GITHUB_TOKEN` 具备 `contents: read`（同仓库足够）；跨仓库复用才受 public 限制。
5. 给 `libwdi-static` 分支开 environments 保护可选（发布 job 需要 `contents: write`，建议加 required reviewer）。

---

## 7. 验收标准

### 7.1 `libwdi-static` 分支自测（"完成"的定义）

一次 `libwdi-static.yml` 运行必须全部满足：

1. **构建**：`tools/build-libwdi.ps1` 退出码 0；`msbuild` 4 个目标无 MSB8013/MSB8020。
2. **产物存在**：`dist/lib/libwdi.lib` 存在，大小 > 1 MB。
3. **SHA256**：`libwdi.lib` 的 SHA256 与 `BUILD-INFO.json` 的 `lib_sha256`、`SHA256SUMS` 三者一致；该值写入分支 README 与产品仓库常量。
4. **符号齐全**：`dumpbin /symbols` 能找到 6 个符号：`wdi_create_list`、`wdi_destroy_list`、`wdi_prepare_driver`、`wdi_install_driver`、`wdi_strerror`、`wdi_set_log_level`。
5. **x64-only / WinUSB-only**：`BUILD-INFO.json.payload` 明确记录；产物只来自 `libwdi\x64\Release`，不含 `installer_x86` 载荷。
6. **静态 CRT**：`libwdi.lib` 的 `.drectve` 含 `LIBCMT`、不含 `MSVCRT/LIBCMTD`（启发式，佐证）；**权威判据**是产品 exe 的 `dumpbin /dependents` 没有 `vcruntime*`/`msvcp140*`/`ucrtbase.dll`/`api-ms-win-crt-*`（`7.2`）。
7. **许可**：`dist/COPYING-LGPL` 存在且是 LGPLv3 文本；`LICENSES/LGPL-3.0.txt` 与上游 clone 一致。
8. **链接冒烟**：`consume-smoke.yml` 在 windows-2022 上 `cargo test --no-run` 通过（证明 `libwdi.lib` + 19 个系统库可链接）。
9. **消费冒烟**：`consume-smoke.yml` 的两个消费者路径都通过——B（下载 asset + hash 校验）与 C（外部空 crate `cargo metadata` 能解析 `libwdi-sys`）。
10. **发布**：tag `libwdi-1.5.1-x64-r1` 的 `release` job 产出 `libwdi-1.5.1-x64.zip` + `SHA256SUMS` + `BUILD-INFO.json`，asset SHA256 回填到产品仓库 PR。

### 7.2 main 接入后的验证（Cut1）

```powershell
# 1) Windows job 绿，且不再有 libwdi 构建 step
gh run view <run-id> --log

# 2) 产物内容
Expand-Archive dist/auroku-windows-x86_64-v0.3.1.zip -DestinationPath .check -Force
Get-ChildItem .check          # 期望含 auroku.exe、auroku-gui.exe、LICENSE-LGPL-3.0.txt

# 3) PE 导入表：无动态 libwdi/libusb/CRT
dumpbin /nologo /dependents .check/auroku.exe
dumpbin /nologo /dependents .check/auroku-gui.exe
# 期望：只有 kernel32/user32/... 等 OS DLL；无 libwdi.dll、libusb-1.0.dll、vcruntime140*.dll、
#       msvcp140*.dll、ucrtbase.dll、api-ms-win-crt-*.dll

# 4) 静态链接证据：libwdi 符号已解析进 exe
dumpbin /nologo /symbols .check/auroku.exe | Select-String 'wdi_install_driver'
# 期望：不再出现 UNDEF（被链接进来）；配合 3) 无 libwdi.dll => 静态成功

# 5) 离线/本地可复现（可选）：设置 LIBWDI_LIB_DIR 后
$env:LIBWDI_LIB_DIR = "<解压后的>/lib"
cargo build --locked --features gui,static-libwdi --bins
```

### 7.3 main 接入后的验证（Cut2，若执行）

```powershell
cargo tree -p auroku --features static-libwdi | Select-String 'libwdi-sys'
# 期望：libwdi-sys v0.1.0 (https://github.com/lazybri/aurokku-rs?tag=libwdi-1.5.1-x64-r1#<rev>)
cargo build --locked --features gui,static-libwdi   # --locked 必须通过 => Cargo.lock 已提交
cargo metadata --locked | Out-Null
```

### 7.4 qt6 接入后的验证

见 `5.4 命令。必须额外满足：

1. `build-windows.ps1` 在 **没有** `.libwdi` 目录时给出清晰错误（而不是链到一半失败）；有产物时构建成功。
2. `auroku-gui.exe` 与 `auroku.exe` 的导入表都只有 OS DLL（重点：GUI 此前从未被检查）。
3. zip 内包含 `LICENSE-LGPL-3.0.txt`；`--smoke-test` 仍通过（`qt6 build.yml:121-130` 的 offscreen 冒烟路径不变）。
4. 移除 `-DAUROKU_STATIC_LIBWDI=ON` 时，构建回退为"无内置驱动"行为，且不引入新依赖（可复现旧基线）。

---

## 8. 未决问题（需要 Lead/维护者决定）

1. **是否接受预编译二进制进入发布链**（B 方案）：若组织/项目原则要求"发布物必须由产品仓库 CI 从源码构建"，则主方案改为 A，并接受每次构建 WDK/MSBuild 成本。
2. **qt6 是否真的需要驱动安装能力**：如果 qt6 只是实验性前端，可只做 Cut1 的 main 部分，qt6 暂不接入（避免 C/CMake 复杂度）。
3. **Cut2 的 crate 命名**：`libwdi-sys` 是否可能与 crates.io 上同名包冲突（即便 `publish = false`，依赖图里重名不冲突，但文档需说明）；备选 `auroku-libwdi-sys`。
4. **libwdi 版本升级策略**：是否要求每次升级都在 `libwdi-static` 发新 `r<N>` 并同步两个产品分支？还是允许 main 先行？
5. **是否升级到 libwdi 1.5.1 之后的版本**：本文未联网核实上游最新版本（见附录 B），需在实施时用 `git ls-remote --tags` 确认。
6. **产物是否附带上游源码 tarball**：LGPL 合规更稳妥，但会增加 asset 体积；可放 `UPSTREAM-SOURCE.txt` 指向 GitHub commit 而非打包源码。
7. **是否需要在产品仓库保留"本地构建 libwdi"的逃生通道**：本文沿用了 `docs/windows-driver.md:179-180` 的"仓库不提供本地构建入口"，但维护者可能希望有一个 `tools/fetch-libwdi.ps1` 供离线开发（不放进产品仓库也可文档说明）。

---

## 附录 A：证据索引（只读命令与结果摘要）

### A.1 仓库/分支/标签

```text
$ git -C main show-ref
b219bd5... refs/heads/main
51ff33b... refs/heads/qt6
b219bd5... refs/remotes/origin/HEAD
b219bd5... refs/remotes/origin/main
51ff33b... refs/remotes/origin/qt6
a9df80e... refs/tags/v0.0.1
777c2d2... refs/tags/v0.1.0
777c2d2... refs/tags/v0.1.0-qt6
b9e51cc... refs/tags/v0.2.0
515ea6e... refs/tags/v0.2.0-qt6   (附注标签)
77e1064... refs/tags/v0.3.1
```

- `git cat-file -t 515ea6e` -> `tag`（附注标签）；解引用到 `e21f816`（`v0.2.0-qt6` 提交）。
- `git merge-base main qt6` -> `777c2d2`（与任务给出的共同祖先一致）。
- `git rev-list --count 777c2d2..main` -> `23`；`777c2d2..qt6` -> `10`。
- `git log --oneline v0.3.1..main` -> 仅 `b219bd5`（`ci: debug builds on every push, releases only via manual dispatch`），只改 `build-macos.sh/build.yml/README*`（`git diff --stat v0.3.1..main`）。

### A.2 libwdi 相关提交

```text
$ git -C main log --oneline -S libwdi --all -- .github/workflows/build.yml .github/scripts/build-windows.ps1 docs/windows-driver.md
ca8b2d7 ci(windows): build an x64-only static libwdi
108a129 feat(windows): bundle a static libwdi and verify the static CRT in CI
```

`git show --stat ca8b2d7` -> 仅 `.github/workflows/build.yml`（19+/7-），即 x64-only 改造全部在 workflow 内联正则里（支持 D2 结论）。

### A.3 main↔qt6 合并冲突（说明不能靠合并共享）

```text
$ git -C main merge-tree --write-tree --name-only main qt6
24d0664...
.github/scripts/build-linux.sh
.github/scripts/build-macos.sh
.github/workflows/build.yml
Cargo.lock
Cargo.toml
README.md
README.zh-CN.md
build-windows.ps1
src/bin/auroku-gui/{app,fonts,i18n,main,worker}.rs
src/lib.rs

CONFLICT (content): Merge conflict in .github/scripts/build-linux.sh
CONFLICT (content): Merge conflict in .github/scripts/build-macos.sh
CONFLICT (content): Merge conflict in .github/workflows/build.yml
CONFLICT (content): Merge conflict in Cargo.lock
CONFLICT (content): Merge conflict in Cargo.toml
CONFLICT (content): Merge conflict in README.md
CONFLICT (content): Merge conflict in README.zh-CN.md
CONFLICT (modify/delete): build-windows.ps1 deleted in main and modified in qt6
CONFLICT (modify/delete): src/bin/auroku-gui/app.rs deleted in qt6 and modified in main
CONFLICT (modify/delete): src/bin/auroku-gui/fonts.rs ...
CONFLICT (modify/delete): src/bin/auroku-gui/i18n.rs ...
CONFLICT (modify/delete): src/bin/auroku-gui/main.rs ...
CONFLICT (modify/delete): src/bin/auroku-gui/worker.rs ...
CONFLICT (content): Merge conflict in src/lib.rs
```

### A.4 qt6 无 libwdi 的 grep 证据

```text
$ grep -Ei "libwdi|WinUSB|wdi|install-driver" C:/.../aurokku-rs.worktrees/qt6
(0 matches)          # 而同一 grep 在 main 命中 80+ 行
```

qt6 `ls-tree -r --name-only HEAD` 中 `crates/` 只有 `auroku-ffi`；`src/` 只有 `lib.rs`、`main.rs`。

### A.5 关键文件行号核对

- `.github/workflows/build.yml`：Windows 矩阵 `80-83`；`setup-msbuild` `108-109`；libwdi step `115-157`；WDK `120-121`；clone `122`；config.h `126-134`；宏 `136-141`；msbuild `143-146`；工程引用 `147-153`；静态库 `155-157`；产品构建 `159-162`；上传 `200-205`；Release `207-243`。
- `.github/scripts/build-windows.ps1`：静态校验 `39-91`；banned 列表 `78-85`；crt-static `101-106`；feature 开关 `114-121`；`cargo build` `135-136`；校验调用 `155-161`；许可 `163-171`；zip `173-175`。
- `crates/auroku-driver-sys/build.rs`：`:8` rerun；`:10-16` 缺失警告；`:18-19` link search/static；`:24-30` 19 个 dylib。
- `docs/windows-driver.md`：程序内安装 `36-79`；构建 `177-191`；许可证 `193-195`。
- qt6：`gui/CMakeLists.txt:59-69`（cargo staticlib + imported target）、`:139-144`（链接系统库）、`:151-152`（cmake 调用）、`:171-181`（windeployqt）；`build-windows.ps1:62-99`（CLI + 校验）、`:241-249`（打包）；`.github/workflows/build.yml:99-108`（MSVC/Qt）、`:116-130`（Windows 构建/冒烟）。

---

## 附录 B：未验证项（实施时必须补齐）

1. **上游 `config.h` 精确内容/行号**：本环境网络受限（`raw.githubusercontent.com` 被解析到非公网地址，`web_fetch` 被拒绝），无法抓取 libwdi v1.5.1 的 `msvc/config.h` 与 `libwdi_static.vcxproj` 原文。实施时用 `git show <sha>:msvc/config.h` 与 `git show <sha>:libwdi/.msvc/libwdi_static.vcxproj` 生成 patch 并核对本表的 4 项替换与工程引用删除。
2. **上游最新版本**：未联网核实 v1.5.1 之后是否有新 tag/安全修复；实施前跑 `git ls-remote --tags https://github.com/pbatard/libwdi.git`。
3. **CI 耗时**：无实测数据（本文 `6.3 给了测量字段）。
4. **`LIBCMT` 字符串探测**：是启发式，需用真实 `libwdi.lib` 验证；权威判据仍是产品 exe 的 `dumpbin /dependents`。
5. **`windows-2022` 上 `libwdi_static.vcxproj` 的 `PlatformToolset`**：未在本机/CI 验证；若 MSB8020，加 `0003` patch 或改用镜像已有工具集。
6. **`gh release download` 在本仓库的权限行为**：未实跑；同仓库 public 仓库预期 `contents: read` 足够，实施时先用 `workflow_dispatch` 验证。
7. **驱动签名策略**：本文只讨论构建解耦，不涉及驱动签名（`docs/windows-driver.md:146-148` 的签名要求保持现状）。

---

## 附录 C：一页式改动清单（给 Lead 执行时对照）

| 序 | 仓库/分支 | 动作 | 依赖 |
| --- | --- | --- | --- |
| 1 | 新建 `libwdi-static`（orphan） | 放入 `3.2` 全部文件；`crates/libwdi-sys` 先放骨架（Cut1 可暂空实现，Cut2 再迁移完整绑定） | 无 |
| 2 | `libwdi-static` | 生成 `patches/0001`、`0002`；填 `upstream/libwdi.ref`、`BUILD-INFO.json` 的 SHA 常量 | 1 |
| 3 | `libwdi-static` | 跑通 `libwdi-static.yml` + `consume-smoke.yml`；发 tag `libwdi-1.5.1-x64-r1` | 2 |
| 4 | main | `build.yml` 删 `108-157`，加下载+校验 step；`build-windows.ps1` 改许可来源 | 3 |
| 5 | main | 更新 `docs/windows-driver.md:177-195`、`README.md:240-243`、`README.zh-CN.md:216-218` | 4 |
| 6 | main | 加 Windows link-smoke job（`pull_request` 路径） | 4 |
| 7 | qt6（可选） | 按 `5.2` Q1–Q9 接入；先 Cut1 | 3 |
| 8 | main+qt6（可选） | Cut2：crate 迁到分支 → git tag 依赖 → 提交 `Cargo.lock` → 删本地 crate | 6/7 |



