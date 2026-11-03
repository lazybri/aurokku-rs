# main 分支 v0.2.0→b219bd5 提交史与「汇入 qt6」移植单元清单

> 作者：teammate **historian**（只读分析）。生成于 2026-11-04。
> 分析对象：main 工作树 **C:/Users/Administrator/Documents/proj/aurokku-rs**，HEAD = **b219bd5**（origin/main 同步）；qt6 工作树 **C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6**，HEAD = **51ff33b**（仅做只读核对，qt6 细节以 02-qt6-gap-analysis.md 为准）。
> 约束遵守：全程只用只读 git（log/show/diff/numstat/summary/rev-list/grep），未 checkout/merge/commit，未改动两个工作树的任何文件，未运行 cargo（避免写 target/）。
> 所有 `file:line` 均指 main HEAD（b219bd5）中的文件内容。

---

## 0. 结论摘要（先看这 10 条）

1. **提交数是 19 个，不是 22 个**。`git rev-list --count v0.2.0..b219bd5` = **19**；v0.1.0..v0.2.0 = 4；共同祖先 777c2d2..b219bd5 = **23**（4+19）。任务书的「22」无法由任何 ref 区间复现，正文按实测 19 + 基线 4 全覆盖，确保不漏提交。
2. 这一段的**唯一功能主线**是：Windows 上设备被 usbser.sys 整机占用 → 用 libwdi 在进程内装 WinUSB 驱动（发布版静态链接 libwdi；无 libwdi 时回退外部 wdi-simple.exe），并支持恢复原驱动、崩溃恢复、driver.log、UAC 提权。其余提交都是这条主线的修 bug/CI/文档。
3. **最大的一次提交是 108a129（19 文件，+1619/−129）**，它同时改 Rust 核心、新增 driver 层、新增 bindings crate、搬移构建脚本、改 CI、改 egui GUI、写新文档。**因此不能按 commit 粒度 cherry-pick，必须按文件/功能块（P1..P10）移植。**
4. **Rust 侧几乎全是「新增文件 + 新增 pub 项」**：`src/driver.rs` 719 行新文件、`crates/auroku-driver-sys` 3 个新文件、`src/lib.rs` 仅 +364/−4。没有改名/删函数；唯一的半破坏点是在 `AuRokkuError` 新增 `Driver` 变体（外部穷举 match 会编译失败，qt6 的 FFI 只用 `kind()`，不受影响）。
5. **egui GUI 的驱动安装/恢复/日志等新交互（5.3 节列了 9 项）在 qt6 必须用 C++ 重写**：`src/bin/auroku-gui` 在 qt6 已被整体删除（qt6 diff 中 app.rs −2583 等），所以 GUI 改动零文本冲突、但全部是「语义移植」。
6. **FFI 需要新增 4~5 个入口**（install/restore/pending_restore/log_path，外加 `DriverError` kind 透传），qt6 现有 `crates/auroku-ffi/src/lib.rs` 只有 4 个 extern C（version/scan_devices/run_task/string_free）。
7. **冲突面集中在 4 个文件**：`src/lib.rs`（qt6 同区域另有 141 行改动，且把 `usb_location` 改名 `device_location`、另写了 sysfs 名称回退）、`Cargo.toml`（qt6 是 workspace+auroku-ffi、无 gui/eframe/static-libwdi）、`.github/workflows/build.yml`、`.github/scripts`（qt6 仍用根目录 build-*.sh/ps1，且内容已改成 CMake/Qt 构建）。
8. **CI 语义被重写**：构建脚本从仓库根搬入 `.github/scripts/`（根 `build-windows.ps1` 删除）、Windows job 内联「解包 WDK + 编 libwdi 静态库」约 100 行、触发器从「tag 才发布」变为「每次 push 都编 debug + 只有手动 dispatch release 才发布」。这三件事对 qt6 都是「结构级」改动，不是补丁级。
9. **版本号冲突**：main 0.3.1（77e1064）vs qt6 0.2.0-qt6；且 b219bd5 在 v0.3.1 之后、只改 CI/README（4 文件 +74/−48），必须单独作为一个发布策略补丁计入移植计划。
10. 建议移植顺序：**P1→P2→P3→P4**（Rust/CLI 先落地并可独立编译）→ **P9/P10**（FFI + C++ GUI，可与 CI 并行）→ **P5→P6→P7**（CI 最后，且要与 qt6 的 CMake/Qt 构建合并）→ **P8 文档**。

---

## 1. 提交清单（19 个，按时间正序）

组别：A=Rust 核心，B=Windows 驱动安装层，C=构建/CI，D=egui GUI，E=版本/发布。± 行为该提交自身的 `--shortstat`。

| # | 组 | commit | 时间(+0800) | 主题（原文） | 主要涉及文件 | ±行 |
|---|---|---|---|---|---|---|
| 1 | A/B/C/D | `108a129` | 11-04 01:27 | feat(windows): bundle a static libwdi and verify the static CRT in CI | 19 文件：`src/driver.rs`(新 552)、`src/lib.rs`(+150/−4)、`crates/auroku-driver-sys/*`(新 3 文件)、`src/bin/auroku-gui/{app,worker,i18n}.rs`、`Cargo.toml`、`Cargo.lock`、`.github/scripts/build-windows.ps1`(新 164)、`.github/workflows/build.yml`、`docs/windows-driver.md`(新 191)、`README*.md`、脚本搬移 | **+1619/−129** |
| 2 | B | `b66d3d6` | 11-04 01:30 | fix(ci): gate Windows-only driver imports so Linux clippy stays clean | `src/driver.rs`（`Duration/SystemTime/now_unix` 加 `cfg(windows)`） | +2 |
| 3 | C | `ca8b2d7` | 11-04 01:40 | ci(windows): build an x64-only static libwdi | `.github/workflows/build.yml`（config.h 关 OPT_M32/OPT_ARM；先编 detect_64build/installer_x64/embedder，再编 libwdi_static，BuildProjectReferences=false） | +19/−7 |
| 4 | C | `30994bc` | 11-04 01:44 | ci(windows): pass the WDK path with forward slashes | `build.yml`（WDK_DIR 反斜杠是 MSVC 转义 → 换正斜杠） | +4/−1 |
| 5 | C | `bba951e` | 11-04 01:48 | ci(windows): embed only the WinUSB payload | `build.yml`（注释掉 LIBUSB0_DIR/LIBUSBK_DIR 首轮） | +4 |
| 6 | C | `b4afdb2` | 11-04 01:52 | ci(windows): comment out the whole LIBUSB0_DIR/LIBUSBK_DIR defines | `build.yml`（改成整行正则替换，避免残留路径字符串） | +2/−2 |
| 7 | C | `ac7494a` | 11-04 01:56 | ci(windows): drop libwdi_static project references at build time | `build.yml`（运行期剥离 `<ProjectReference>`，修 MSB8013） | +9/−3 |
| 8 | B | `625f489` | 11-04 02:01 | fix(windows): link shell32 and the other libwdi import libraries | `crates/auroku-driver-sys/build.rs`（+19 个系统 import lib） | +2/−1 |
| 9 | B | `7f5ca5e` | 11-04 02:15 | fix(windows): generate the INF before calling wdi_install_driver | `crates/auroku-driver-sys/src/lib.rs`（先 wdi_prepare_driver 再 wdi_install_driver） | +65/−2 |
| 10 | B | `66ad7d3` | 11-04 02:29 | fix(windows): pass the device hardware ID to libwdi | 同上（wdi_create_list 查 hardware_id/device_id） | +65/−2 |
| 11 | B | `a38115b` | 11-04 02:43 | fix(windows): delete the libwdi driver package when restoring | `src/driver.rs`（pnputil 删 oemNN.inf + remove-device + scan-devices） | +59/−22 |
| 12 | B | `05bd642` | 11-04 02:53 | fix(windows): match UTF-16 INF files when looking for the driver package | `src/driver.rs`（inf_mentions 同时搜 ASCII 与宽字符） | +20/−4 |
| 13 | A | `ccd4447` | 11-04 03:15 | feat(windows): resolve device names by VID:PID when descriptors are unreadable | `src/lib.rs`（Windows 设备树名字回退 + 每 scan 缓存） | +122 |
| 14 | A | `b5e78b7` | 11-04 03:17 | fix(ci): mark the Windows-only name decoder as allowed on other targets | `src/lib.rs`（`cfg_attr(not(windows), allow(dead_code))`） | +1 |
| 15 | A | `78687fa` | 11-04 03:25 | fix(windows): decode non-ASCII device names from the registry | `src/lib.rs`（U+FFFD → PowerShell UTF-8 重读） | +78 |
| 16 | B/D | `c9a75d4` | 11-04 03:47 | feat(windows): log driver operations to driver.log and hide the elevated window | `src/driver.rs`(+126/−14)、`src/lib.rs`(+21/−8)、`src/bin/auroku-gui/{app,worker,i18n}.rs`、`docs/windows-driver.md` | +160/−25 |
| 17 | D | `ba75b4c` | 11-04 04:45 | fix(gui): build release as a Windows-subsystem app and log to gui.log | `src/bin/auroku-gui/main.rs`、`diag.rs`(新 35)、`{app,fonts,permissions,worker}.rs` | +52/−13 |
| 18 | C/E | `77e1064` | 11-04 06:00 | ci: add a debug branch, versioned release archives, bump to 0.3.1 | `build.yml`、`.github/scripts/build-{linux,macos,windows}.*`、`Cargo.toml`/`Cargo.lock`（0.2.0→0.3.1） | +93/−37 |
| 19 | C/E | `b219bd5` | 11-04 06:10 | ci: debug builds on every push, releases only via manual dispatch | `build.yml`、`.github/scripts/build-macos.sh`、`README.md`、`README.zh-CN.md` | +74/−48 |

**b219bd5 在 v0.3.1 之后**：`git log --oneline --decorate` 为 `b219bd5 (HEAD -> main, origin/main) / 77e1064 (tag: v0.3.1)`，即 77e1064 是 b219bd5 的父提交；`git diff --stat v0.3.1..b219bd5` 只有 4 个文件（CI×2 + README×2），无源码。移植清单里必须把它作为「发布策略补丁」单列（见 P7）。

**逐提交 ± 合计 +2450/−296，而区间净 diff 只有 +2357/−203**：差值来自同一文件被多轮覆盖（如 `.github/scripts/build-windows.ps1` 在 108a129 建 164 行、77e1064 再改到 180 行；`src/driver.rs` 552→719 行）。做工作量估算时以净 diff 为准，做过程复现时以逐提交为准。

---

## 2. 分组分析（逐提交：解决了什么问题 → 关键 diff → 副作用）

### A. Rust 核心层（`src/lib.rs`；`src/main.rs` 零改动）

> 核实：`git diff --numstat v0.2.0..b219bd5` 中不存在 `src/main.rs`；CLI 入口完全复用 `src/lib.rs` 的 `pub fn cli_main()`（`src/lib.rs:2814`）。

**A-1 `108a129`（lib.rs 部分，+150/−4）—— 给 CLI 加驱动操作并让「无 WinUSB」错误可指导用户**
- 新增 `AuRokkuError::Driver(String)`（`src/lib.rs:51`）与 `kind()` 返回 `"DriverError"`（`src/lib.rs:61`）；
- 新增私有 `open_failure(err, vid, pid)`（`src/lib.rs:85`）：Windows 上 `NotSupported/Access` 转成带操作建议的 `DriverError`（提示装 WinUSB、给 `--op install-driver` 命令、提 Zadig 与恢复命令）；`ensure_handle` 的 `open` 失败改走它（`src/lib.rs:641`）；
- `Op` 枚举加 `InstallDriver` / `RestoreDriver`（`src/lib.rs:2269/2272`，CLI 名 `install-driver`/`restore-driver` 见 :2290/:2291）；
- `Cli` 加 `--restore-on-exit`（:2364）与隐藏参数 `--driver-log-append`（:2368，c9a75d4 引入）；
- `print_human_report` 增加两个 op 的人读输出（`src/lib.rs:2427` 函数内 install-driver 分支约 :2627）；帮助文本加两条示例（约 :2711）；
- `task_from_cli` 对这两个 op 是 `unreachable!`（:2741），实际由 `run_driver_op(cli) -> i32`（:2748）提前分流，`cli_main` 在 :2825-2826 直接 `std::process::exit`；
- `cli_main` 启动时检查 `driver::pending_restore()` 并打 [WARN]（:2817-2823）——这是崩溃恢复提示。
- **副作用**：`AuRokkuError` 多一个变体；任何 crate 外穷举 match 会失效（仓库内无此情况；qt6 的 FFI 通过 `kind()` 取字符串，天然兼容）。`open_failure` 的消息里硬编码英文，与 GUI 的中文提示并存。

**A-2 `ccd4447`（+122）—— 设备被别的驱动占用时仍能显示可读名字**
- 问题：设备被 usbser.sys 占用后 libusb 不能 open，GUI 列表只剩「USB 设备 04c5:1270」。
- 关键新增（全部私有，不扩公开 API）：`decode_device_text`（`src/lib.rs:1943`，剥离 `@usbser.inf,%...%;USB Serial Device` 间接串）、`DeviceName` / `DeviceNameCache`（:1957/:1961）、`windows_device_strings`（:1964，`reg query HKLM/SYSTEM/CurrentControlSet/Enum/USB/VID_xxxx&PID_yyyy /s` 解析 FriendlyName/DeviceDesc/Mfg）、`scan_devices` 内接入与每 scan 缓存（:2107-2109、:2156-2166）。
- **副作用**：每次 scan 每个缺失名字的设备最多一次 `reg.exe` 子进程（有缓存）；非 Windows 平台类型/函数全部 `cfg(windows)`，`decode_device_text` 用 `cfg_attr(not(windows), allow(dead_code))`（b5e78b7 补）。

**A-3 `b5e78b7`（+1）**：Linux clippy `-D warnings` 因 `decode_device_text` 只在 Windows 调用而 dead_code 失败 → 加 `allow`。纯 lint 修复。

**A-4 `78687fa`（+78）—— 中文设备名乱码**
- 问题：`reg.exe` 按控制台代码页（GBK）输出，按 UTF-8 读得到 U+FFFD，GUI 显示方框。
- 关键 diff：`has_replacement`（`src/lib.rs:2033`）检测 U+FFFD；`powershell_device_strings`（:2044）用 `powershell -NoProfile -NonInteractive -Command` + `[Console]::OutputEncoding=[Text.Encoding]::UTF8` + `Get-ItemProperty` 重读 FN/DD/MFG；`windows_device_strings` 在检测到替换字符时升级到 PowerShell（:2016-2025）。
- **副作用**：只在命中 U+FFFD 时才多起一个 PowerShell 进程（懒回退）；`Get-ChildItem` 会遍历该 VID/PID 下所有实例，多实例时取第一个非空值。

**A-5 `c9a75d4`（lib.rs +21/−8）**：为提权子进程追加 `--driver-log-append`，并把 `driver::install/restore` 的调用点改成新签名（见 B-6）。

### B. Windows 驱动安装层（`src/driver.rs` 新文件 719 行 + `crates/auroku-driver-sys`）

> 结构：`driver.rs` 是「跨平台外壳 + Windows 具体实现」；`auroku-driver-sys` 是唯一允许 unsafe 的 crate，二者用 feature `static-libwdi` 切换「进程内 libwdi」与「外部 wdi-simple.exe」。

**B-1 `108a129`（driver.rs 552 行 + bindings crate 179+30 行）—— 主实现**
- `driver.rs`：状态文件（`driver-state.json`）、`install()/restore()`（平台 stub + Windows 实现）、`driver_service()`（`reg query ... /v Service` 判断当前功能驱动）、`locate_wdi()` 回退链、`run_elevated()`（`powershell Start-Process -Verb RunAs -WindowStyle Hidden -Wait -PassThru`）、`wait_until()`（30s 轮询 `can_open` / 服务名不再是 WINUSB）、`can_open()`（rusb 重试 open）。
- `auroku-driver-sys`：手写 `wdi_device_info` / `wdi_options_*` `#[repr(C)]` 结构、`wdi_create_list/wdi_destroy_list/wdi_prepare_driver/wdi_install_driver/wdi_strerror/wdi_set_log_level` 声明、`is_elevated()`（OpenProcessToken+GetTokenInformation）、`install_winusb()`、`WdiError`。
- **副作用**：`src/driver.rs` 在非 Windows 也能编译（stub 返回 Err），所以 `pub mod driver` 是无条件的；Windows 上 `install` 会真正弹 UAC。

**B-2 `b66d3d6`（+2）**：`Duration/SystemTime/now_unix` 仅 Windows 安装路径用，无条件 import 让 Linux clippy 失败 → 加 `cfg(windows)`。纯 lint。

**B-3 `625f489`（build.rs +2/−1）**：静态 libwdi 的 obj 引用 `ShellExecuteExW`/`SHCreateDirectoryExW` → LNK2019；build.rs 追加 19 个系统 import lib（setupapi/ole32/ntdll/advapi32/user32/gdi32/comctl32/crypt32/wintrust/version/shlwapi/cfgmgr32/msi/shell32/oleaut32/uuid/comdlg32/winspool/userenv，`crates/auroku-driver-sys/build.rs:24-30`）。

**B-4 `7f5ca5e`（+65/−2）**：`wdi_install_driver` 传 NULL path 会 `WDI_ERROR_INVALID_PARAM` → 学 wdi-simple：建临时工作目录（`%TEMP%/auroku-wdi`），先 `wdi_prepare_driver`（WDI_WINUSB 生成 `usb_device.inf` + cat），再 `wdi_install_driver`（`crates/auroku-driver-sys/src/lib.rs:180-254`）。

**B-5 `66ad7d3`（+65/−2）**：不填 `device_info->hardware_id` 时安装器不调 `UpdateDriverForPlugAndPlayDevices`，包导入了但设备还是 usbser → 用 `wdi_create_list` 按 VID/PID 查真实 `hardware_id`/`device_id`，失败回退 `USBVID_xxxx&PID_yyyy`（`crates/auroku-driver-sys/src/lib.rs:193-214、:260-295`）。

**B-6 `a38115b`（driver.rs +59/−22）+ `05bd642`（+20/−4）—— 恢复必须删驱动包**
- 问题①：只 `pnputil /remove-device` 设备节点，WinUSB 包仍在 driver store，PnP 重新枚举又绑回 WinUSB，看起来恢复成功实则没恢复 → 先枚举 `C:/Windows/INF/oemNN.inf` 找含 `VID_xxxx&PID_yyyy` 的包，逐个 `pnputil /delete-driver <oemNN.inf> /uninstall /force`，再 `/remove-device` + `/scan-devices`；`wait_until(false,...)` 要求服务名存在且非 WINUSB 才算成功（`src/driver.rs:420-444、:498-514、:622-660`）。
- 问题②：libwdi 生成的 INF 是带 BOM 的 UTF-16LE，纯 ASCII 字节搜不到 → `inf_mentions` 同时搜 ASCII 与「每字节补 0」的宽字符（`src/driver.rs:516-533`）。

**B-7 `c9a75d4`（driver.rs +126/−14）—— driver.log 与静默提权**
- 新增 `DriverLog`（`Arc<dyn Fn(&str)>`，`src/driver.rs:224`）、`log_path()`（:227）、`reset_log()`（:264，每次用户操作截断重写）、`emit()`（:281，先写文件再走 sink 或 stderr）、时间戳 `HH:MM:SSZ`（:233）；`install/restore` 签名扩为 5/4 参数（追加 `log` 与 `append_log`，:293/:337）；`run_elevated` 改为 `Start-Process ... -WindowStyle Hidden`（:590-618）。
- **副作用**：日志写失败永不阻断安装（`let _ =`）；时间戳只有「当天秒数折算」，没有日期（:238-244）；`reset_log` 的 `format!` 里嵌了真实换行（:272-275），可读性差但合法。

**B-8 小缺陷（移植时可顺手修，非阻塞）**：`src/driver.rs:360-367` 非 Windows 的 `restore` stub 复用了「installation is only supported on Windows」文案（措辞错）；`src/driver.rs:652` 错误串里有一大段连续空格（格式化残留）；`docs/windows-driver.md:71-72` 与 `:74-75` 是逐字重复的段落（108a129 引入）。

### C. Windows 构建与 CI（`Cargo.toml`、`.github/workflows/build.yml`、`.github/scripts/build-windows.ps1`、文档）

**C-1 `108a129` —— 构建入口从仓库根收进 CI，并静态链接 libwdi**
- `Cargo.toml`：新增 feature `static-libwdi = ["dep:auroku-driver-sys"]`（现 `Cargo.toml:16`）、Windows 可选路径依赖（:35-37）、`[workspace] members = ["crates/auroku-driver-sys"]`（:43-44）。
- 脚本重排（`git diff --summary`）：`build-linux.sh => .github/scripts/build-linux.sh`（86% 相似，mode 100755→100644）、`build-macos.sh => .github/scripts/build-macos.sh`（75%）、新建 `.github/scripts/build-windows.ps1`（164 行，替代被删的根 82 行脚本）、`windows-cross-build-linux.ps1 => .github/scripts/...`（95%）。
- `.github/workflows/build.yml`：Windows job 增加 `microsoft/setup-msbuild@v2` + 内联 libwdi 构建（下 WDK redist MSI → `msiexec /a` 解包不安装 → clone libwdi v1.5.1 → msbuild 静态库 → 导出 `LIBWDI_LIB_DIR`）；三个平台步骤改指 `.github/scripts/`。
- `.github/scripts/build-windows.ps1`：`crt-static` 强制静态 CRT（:101-106）；`LIBWDI_LIB_DIR` 存在时自动用 `gui,static-libwdi` 特性（:117-121）；`Assert-StaticRuntime` 用 `dumpbin /dependents` 禁动态 CRT/同名 DLL（:57-91）；LGPL 合规复制（:163-171）；产物名带版本号（:173-175）。
- 文档：`docs/windows-driver.md` 新增 195 行（现象/拓扑/4 种安装方式/验证/恢复/FAQ/构建/许可证）；README 中英同步（+56/−29、+49/−26）。
- **副作用**：本地不再有根目录构建入口（README 明说「only in GitHub Actions」）；`.github/scripts/build-linux.sh` 的 git 权限位是 644，CI 里显式 `chmod +x`（`build.yml:167`）。

**C-2 `ca8b2d7` → `ac7494a`（5 连修）—— 让 libwdi 静态库在 MSBuild 里真的编出来**
- ca8b2d7：直接编 vcxproj 丢了 solution 平台映射，`embedder/installer_x86` 被当成 x64 → MSB8013；改为关 OPT_M32/OPT_ARM、显式先编 detect_64build + installer_x64 + embedder(Win32)，再编 libwdi_static 且 `BuildProjectReferences=false`；
- 30994bc：WDK_DIR 里的反斜杠被 MSVC 当转义（`a`/`w`/`W`）→ 路径损坏，改正斜杠；
- bba951e/b4afdb2：config.h 默认 `LIBUSB0_DIR/LIBUSBK_DIR` 指向开发者机器，embedder 找不到 → 先只注释定义名（残留路径字符串导致 config.h 解析坏），再改成整行正则替换；
- ac7494a：`BuildProjectReferences=false` 仍会让 MSBuild 解析引用工程 → 运行期用正则剥掉 `<ProjectReference>`，工具工程改为前面显式构建。
- **副作用**：CI 依赖 libwdi 上游 v1.5.1 的工程结构 + WDK 8.0 载荷布局；任一上游变动都会让这段失效（属易腐代码，见风险 R5）。

**C-3 `77e1064` / `b219bd5`**：见 E 组。

### D. egui GUI（`src/bin/auroku-gui/*`；qt6 已删除该目录）

**D-1 `108a129`（app.rs +154、worker.rs +43/−1、i18n.rs +26）—— 驱动安装 UI 的全部交互骨架**
- 状态：`driver_offer: Option<(u16,u16)>`、`retry_after_driver: Option<(Task,String)>`、`restore_driver_on_exit: bool`、`last_target`（现 `app.rs:191-201`）；
- 事件：`worker::Request::{InstallDriver,RestoreDriver}`（`worker.rs:47/53`）与 `Event::Driver{operation,vid,pid,result}`（:78 起），worker 线程里同步调 `driver::install/restore` 后回传（:160-183）；
- 触发：任务失败且 `error.kind == "DriverError"` 时把 `last_target` 塞进 `driver_offer`（`app.rs:450-455`）；启动时 `pending_restore` 也塞进去（:291-299）；
- 展示：结果卡片底部 `driver_actions()`（:1311-1360）——说明文案 + 「安装 WinUSB 驱动」+「恢复原驱动」+ 复选框「退出时自动恢复原驱动」；
- 自动重试：安装成功后 `retry_after_driver.take()` → `start(task,label)`（:479-481，压栈见 :626-627）；
- 退出：`on_exit` 里若 `pending_restore` 存在则 `driver::restore(..., None, false)`，失败只记日志（:2786-2806）。

**D-2 `c9a75d4`（+5/−1 等）**：安装/恢复前把 `driver::log_path()` 打进 GUI 控制台（`app.rs:645-647、:661-663`），新增 i18n 键「驱动日志：{path}」；`install/restore` 调用改成新签名。

**D-3 `ba75b4c`（6 文件）—— 发布版无控制台 + gui.log**
- `main.rs:5` 加 `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`；新增 `diag.rs`（35 行：`OnceLock<Mutex<Option<File>>>` 写 `<state_dir>/gui.log`，启动时截断，每行同时 `eprintln!`）；原 `eprintln!("[auroku-gui] ...")` 全部改成 `crate::diag::log(...)`（fonts/permissions/worker/app）。
- **注意**：这里没有「后端逻辑」——真正装驱动/恢复/日志文件全在 `driver.rs`；GUI 只做状态机与文案。

### E. 版本 / 发布（77e1064、b219bd5）

**E-1 `77e1064`（+93/−37）—— 版本 0.3.1 + 版本化产物 + debug 分支**
- `Cargo.toml` 0.2.0→0.3.1，`Cargo.lock` 同步（`Cargo.lock` 中 `auroku` 版本与新增 `auroku-driver-sys 0.1.0`）；
- 三个构建脚本加 `AUROKU_BUILD_DEBUG=1` / `--debug` / `-Debug` 分支（debug profile、产物后缀 `-debug`）；zip/tar 名带 crate 版本（如 `auroku-windows-x86_64-v0.3.1.zip`）；
- `build.yml` 增加 `debug` 分支触发、矩阵 `asset` → `glob`（`dist/*.zip`、`dist/*.tar.gz`），artifact 名与文件名都按 debug 加后缀。
- **副作用**：Linux/Windows 的产物不再固定名，下载脚本/文档需同步（README 已改）。

**E-2 `b219bd5`（+74/−48，v0.3.1 之后）—— 发布策略反转**
- 触发：去掉 tag 触发与 `debug` 分支，改为 `push: branches: ["**"] + tags-ignore: ["**"]`；`workflow_dispatch` 增加 `mode: release|debug` 选项与可选 `version`；
- `AUROKU_BUILD_DEBUG` 改为「push 或 mode==debug 即 1」；`release` job 条件收紧为 `workflow_dispatch && mode == release`；
- macOS debug 构建在 bash 3.2 + `set -u` 下空数组展开报错 → `RELEASE_FLAG` 从数组改标量（`.github/scripts/build-macos.sh` +8/−6）；
- README 中英改「两模式」说明。
- **副作用**：从「打 tag 发布」变成「手动 dispatch 发布」，与 qt6 的「qt6 分支 push 构建」策略直接冲突（见 R6）。

---

## 3. 公开 API 变化清单（file:line 指 main HEAD）

### 3.1 `src/lib.rs`（crate 根，公开面）

| 项 | 行 | 签名 / 变化 | 用途 |
|---|---|---|---|
| `pub mod driver;` | 24 | 新增模块 | 暴露驱动安装层（整个 driver 模块为 pub，见 3.2） |
| `AuRokkuError::Driver(String)` | 51 | 新增变体 | 「设备枚举到但没绑 WinUSB」的可指导错误 |
| `AuRokkuError::kind()` | 56-63 | 新增分支 `=> "DriverError"` | FFI 错误信封的 `kind` 字段透传（qt6 已按 `kind()` 取值） |
| `fn open_failure(...)` | 85-104 | 私有新函数 | Windows `NotSupported/Access` → `DriverError` + 操作建议 |
| `scan_devices(...)` | 2096 | 签名不变，行为新增 Windows 设备树名字回退（:2156-2166） | GUI/FFI 列表可读名 |
| `Op::InstallDriver/RestoreDriver` | 2269/2272 | 新 CLI 值（`--op install-driver` / `--op restore-driver`） | 等价 GUI 按钮的 CLI 入口 |
| `Cli::restore_on_exit` | 2364 | 新 `--restore-on-exit` | 安装时记录「退出时恢复」意图 |
| `Cli::driver_log_append` | 2368 | 新隐藏 `--driver-log-append` | 提权子进程追加而非截断 driver.log |
| `fn run_driver_op(...)` | 2748 | 私有新函数 | 两个驱动 op 的执行/JSON 输出/退出码 |
| `cli_main()` | 2814 | 行为新增：启动 WARN + 提前分流 | CLI 崩溃恢复提示 |
| `fn decode_device_text / windows_device_strings / has_replacement / powershell_device_strings` | 1943/1964/2033/2044 | **私有**新函数 | Windows 名字解析（不构成公开 API，移植时可原样搬） |
| 测试 | 2911、2930 | `decode_device_text_strips_indirect_strings`、`#[ignore] windows_device_strings_live` | 单测 + 需真机的手工测试 |

**未变化的公开面**（qt6 也依赖）：`DEFAULT_TIMEOUT/DEFAULT_EP_OUT/DEFAULT_EP_IN`、`AuRokkuResult`、`LogSink`、`DeviceConfig`（含 v0.2.0 已有的 `usb_location`）、`Task`、`ProbeKind`、`BatchProfile`、`AuRokkuDevice`、`DeviceSummary`、`scan_devices`、`run_task`、`cli_main`。

### 3.2 `src/driver.rs`（新模块，全部 pub 项）

| 项 | 行 | 签名 | 说明 |
|---|---|---|---|
| `WDI_SIMPLE` / `WDI_ENV` | 24 / 26 | `&str` 常量 | `wdi-simple.exe` / `AUROKU_WDI_SIMPLE` |
| `DriverRecord` | 33-42 | `{vid:u16,pid:u16,installed_at:u64,restore_on_exit:bool,restored:bool}` | 持久化到 `driver-state.json` |
| `state_dir()` / `state_path()` | 75 / 101 | `PathBuf` | 状态目录（`AUROKU_STATE_DIR` 可覆盖；Windows `%LOCALAPPDATA%/auroku`） |
| `load_record()` / `save_record()` / `clear_record()` | 106 / 113 / 128 | `Option<DriverRecord>` / `Result<(),String>` / `()` | 状态读写 |
| `pending_restore()` | 136 | `Option<DriverRecord>` | `restore_on_exit && !restored` —— GUI/CLI 崩溃恢复依据 |
| `locate_wdi()` | 144 | `Option<PathBuf>` | 回退链：env → 程序目录 → 程序目录/tools → 当前目录/tools → PATH |
| `driver_service(vid,pid)` | 182 / 211 | `Option<String>` | 读注册表 `Service`（WINUSB/usbser） |
| `DriverLog` | 224 | `Arc<dyn Fn(&str)+Send+Sync>` | 进度回调（与 `LogSink` 结构相同但不同类型——移植时可统一/桥接） |
| `log_path()` | 227 | `PathBuf` → `<state_dir>/driver.log` | |
| `install(vid,pid,restore_on_exit,log,append_log)` | 293 / 325 | `Result<DriverRecord,String>` | Windows 实装 / 非 Windows 恒 Err |
| `restore(vid,pid,log,append_log)` | 337 / 360 | `Result<(),String>` | 同上 |

内部关键函数（不 pub 但对行为关键）：`install_backend`（:393 static-libwdi / :408 wdi-simple）、`restore_backend`（:426 / :448）、`driver_args`（:462）、`elevate_self`（:477）、`remove_device`（:499）、`inf_mentions`（:521）、`driver_packages`（:540）、`device_instance_ids`（:564）、`run_elevated`（:590）、`wait_until`（:622）、`can_open`（:663）。

### 3.3 `crates/auroku-driver-sys`（新 crate，仅 Windows 编译，`#![cfg(windows)]` + `#![allow(unsafe_code)]`）

| 项 | 行 | 签名 | 说明 |
|---|---|---|---|
| 错误码常量 | 17-27 | `WDI_SUCCESS/WDI_ERROR_IO/WDI_ERROR_INVALID_PARAM/WDI_ERROR_NEEDS_ADMIN(-15)/WDI_LOG_LEVEL_INFO/NONE` | |
| `WdiDeviceInfo` | 31-44 | `#[repr(C)]`，字段含 `vid/pid/desc/driver/device_id/hardware_id/compatible_id` | libwdi.h 镜像 |
| `WdiOptionsInstallDriver` / `WdiOptionsPrepareDriver` / `WdiOptionsCreateList` | 48 / 56 / 70 | `#[repr(C)]` | |
| `WdiError` | 120-125 | `{code:c_int,message:String}+Display+Error` | |
| `is_elevated()` | 136 | `bool` | OpenProcessToken + TokenElevation |
| `set_log_level(level)` | 164 | `()` | 包 `wdi_set_log_level` |
| `install_winusb(vid,pid)` | 175 | `Result<(),WdiError>` | 建 `%TEMP%/auroku-wdi` → prepare INF → install；可能返回 `WDI_ERROR_NEEDS_ADMIN` 让调用者提权 |
| `lookup_ids(vid,pid)` | 260 | 私有 `(Option<CString>,Option<CString>)` | `wdi_create_list` 查 hardware_id/device_id |

`build.rs`：读 `LIBWDI_LIB_DIR`（:10-16），`cargo:rustc-link-search=native={dir}` + `static=libwdi`（:18-19），追加 19 个系统 import lib（:24-30）。

### 3.4 CLI 级 API（对用户/脚本的契约）

- `auroku --op install-driver --vid V --pid P [--restore-on-exit] [--json]`
- `auroku --op restore-driver --vid V --pid P [--json]`
- 隐藏：`--driver-log-append`（提权 helper 内部使用）
- 返回：成功 exit 0 + JSON `{operation,success,vid,pid,restore_on_exit,state_file}`；失败 exit 1 + `{"error":"DriverError","message":...}`（`src/lib.rs:2748-2808`）。

### 3.5 兼容性结论

- 对 Rust crate 使用者：**唯一半破坏**是 `AuRokkuError` 新变体（外部穷举 match 会失败）。仓库内无外部 match；qt6 的 `auroku-ffi` 用 `error.kind()` 与 `Display`（qt6 `crates/auroku-ffi/src/lib.rs:62-69`），**不受影响**。
- 对 C++ 侧：新增 `"DriverError"` 这个 kind 字符串值；C++ 只需识别（现 `ApiError` 是任意字符串，见 qt6 ffi :51-54）。
- 对 CLI 用户：纯增量（两个新 `--op` 值 + 两个新 flag），旧命令不变。

### 3.6 关键调用链（带 file:line）

**安装（发布版，static-libwdi 路径）**
1. 入口：GUI `app.rs:638 install_driver` → `worker.rs:160 Request::InstallDriver` → `worker.rs:166 driver::install(vid,pid,restore_on_exit,None,false)`；或 CLI `lib.rs:2750 run_driver_op -> driver::install`。
2. `driver::install`（`src/driver.rs:293`）→ `install_backend`（`src/driver.rs:393`）→ `auroku_driver_sys::set_log_level(INFO)` → `auroku_driver_sys::install_winusb(vid,pid)`（`crates/auroku-driver-sys/src/lib.rs:175`）。
3. `install_winusb` → 建 `%TEMP%/auroku-wdi`（:181-185）→ `lookup_ids`（:193，内部 `wdi_create_list` :268）→ **`wdi_prepare_driver`（:229-230，生成 usb_device.inf + cat）** → **`wdi_install_driver`（:244-245）**。
4. 若返回 `WDI_ERROR_NEEDS_ADMIN(-15)`（:23/:246-253）→ `driver.rs:398` 分支 `elevate_self`（`src/driver.rs:477`）→ `run_elevated`（:590）启动 `auroku.exe --op install-driver --vid ... --pid ... --driver-log-append`（:462 `driver_args`）→ CLI 里 `run_driver_op` 再走一次同样的 `driver::install`（这次已提权，libwdi 直接成功）。
5. 成功后 `driver::install` 轮询 `wait_until(true, ...)`（`src/driver.rs:309`/:622）——每 400ms 用 `can_open`（:663，rusb 尝试 open）判断；30s 超时报错。最终写 `driver-state.json`（:319 `save_record`）。

**恢复**
1. 入口：GUI `app.rs:654 restore_driver` / CLI `lib.rs:2768`，均调 `driver::restore`（`src/driver.rs:337`）。
2. `restore_backend`（:426）：未提权先 `elevate_self`（:429-432）；提权后 `driver_packages`（:540）扫 `C:/Windows/INF/oemNN.inf`，用 `inf_mentions`（:521，ASCII + UTF-16LE 双搜）匹配 `VID_xxxx&PID_yyyy`；对每个包 `pnputil /delete-driver <inf> /uninstall /force`（:436-441）。
3. `remove_device`（:499）：`device_instance_ids`（:564，`reg query` 解析实例 ID）→ `pnputil /remove-device` → `pnputil /scan-devices`。
4. `wait_until(false, ...)`（:347/:622）：要求 `driver_service`（:182）返回的 Service 存在且不是 WINUSB 才算恢复成功；成功后才把 `DriverRecord.restored` 置 true（:350-354）。

**日志/诊断**
- `driver.log`：`log_path()`（:227）= `<state_dir>/driver.log`；每次用户操作 `reset_log`（:264）截断，提权子进程用 `append_log=true` 追加（`--driver-log-append`）；行格式 `[HH:MM:SSZ] 文本`（:233-245）。GUI 只在控制台打印日志路径，**不消费** `DriverLog` 回调（`worker.rs:166` 传 `None`）。
- `gui.log`：`src/bin/auroku-gui/diag.rs:11 log()`，写 `<state_dir>/gui.log`（:16），启动时截断（:24 `truncate(true)`）；发布版（`not(debug_assertions)`）无控制台（`main.rs:5`）。

---

## 4. 与 qt6 基线的关系（main 侧描述）

### 4.1 v0.1.0..v0.2.0 的 4 个提交（v0.2.0 是本次分析的起点，qt6 在 Rust 侧可能缺）

| # | commit | 做了什么 | 涉及文件与规模 | 对「汇入 qt6」的含义 |
|---|---|---|---|---|
| 1 | `298f8d8` | **无 root 也能显示 USB 设备名**：`scan_devices` 在 Linux 用 `/sys/bus/usb/devices` 补齐 manufacturer/product/serial（按 busnum/devnum 匹配）；GUI 加统一 `device_label` | `src/lib.rs` +92、`gui/app.rs` +59/−17；合计 +134/−17 | main 的 `fill_strings_from_sysfs` 与 qt6 自己的 `read_sysfs_usb_descriptors` 是**同一功能的两次独立实现** → 合并高冲突（见 R1） |
| 2 | `83ef874` | Linux 复核审计报告 + 修 2 个 clippy（permissions.rs 的 `while_let_loop`、`byte_char_slices`） | `AUDIT-REPORT.md` +53、`permissions.rs` 7；+47/−13 | 纯审计/文档 + Linux ACL 代码；qt6 无 `AUDIT-REPORT.md`（只有 `SECURITY_AUDIT.md`），无移植价值 |
| 3 | `3a81f4b` | **按设备的 USB ACL 生命周期清理**：删 `elevate.rs`（−271），新增 `permissions.rs`（+513），`DeviceConfig` 加 `usb_location: Option<(u8,u8)>`，`open_device` 按 bus/address 精确匹配 | 13 文件 +825/−399；`src/lib.rs` 仅 +18 | main 的 `usb_location` 与 qt6 的 `device_location` 是同一改动的两种命名（qt6 在 8c30cfd 那晚独立实现）→ 合并时以 qt6 命名为准、保留 main 语义 |
| 4 | `b9e51cc` | 版本 bump 0.2.0 | 5 文件 +5/−5 | 无 |

### 4.2 关键结论（main 侧）

1. **v0.2.0 不是 qt6 的祖先**：main 从 777c2d2 走到 v0.2.0（10-18..10-30 23:03），qt6 也从 777c2d2 走到 51ff33b（10-21..10-31 05:01）。两者是**平行分支**，v0.2.0 的 4 个提交在 qt6 上要么被独立重写、要么不存在。
2. **时间线说明冲突成因**：qt6 的 `8c30cfd feat(gui): grant per-device USB ACL access`（10-30 22:21）与 `73d3e13 fix(gui): improve USB discovery and polkit waits`（10-30 22:42）几乎与 main 的 `298f8d8`/`3a81f4b` 同一晚完成；随后 main 在 11-04 凌晨做了 Windows 驱动工作，qt6 最后一次提交停在 10-31 05:01，**qt6 从未见过 v0.2.0 之后（尤其 11-04）的任何改动**。
3. 因此「汇入 qt6」实际是**三方合并**：base=777c2d2，一边是 main（23 个提交），一边是 qt6（10 个提交）。本文只负责 main 侧内容与 main 侧风险证据。
4. **只读核对到的 qt6 侧事实**（仅用于风险定级，细节由 02 文档复核）：
   - qt6 `Cargo.toml`：`[workspace] members = [".", "crates/auroku-ffi"], resolver = "2"`，版本 `0.2.0-qt6`，**无** `eframe`/`gui`/`static-libwdi` feature，无 `[[bin]] auroku-gui`；
   - qt6 目录：根目录保留 `build-linux.sh` / `build-macos.sh` / `build-windows.ps1` / `windows-cross-build-linux.ps1`，**无** `.github/scripts/`、**无** `docs/`；
   - qt6 `src/lib.rs` 相对 777c2d2 为 +141/−25：`usb_location` 改名 `device_location`、新增 `read_sysfs_usb_descriptors`、`DeviceSummary::display_name` 改成「制造商 — 产品」；
   - qt6 `crates/auroku-ffi/src/lib.rs`（508 行）暴露 `auroku_ffi_version` / `auroku_scan_devices` / `auroku_run_task` / `auroku_string_free`（:345-449），`FFI_ABI_VERSION = "1"`（:37），错误信封 `{ok,kind,message}`。

---

## 5. 移植单元 P1..P10

### 5.1 总表

| 单元 | 名称 | 含哪些提交 / 文件 | 目标类别 | 建议顺序 | 依赖 | 工作量 | 与 qt6 冲突风险 |
|---|---|---|---|---|---|---|---|
| **P1** | `auroku-driver-sys` 绑定 crate | 108a129（新 3 文件）+ 625f489 + 7f5ca5e + 66ad7d3（**取最终态**） | Rust 核心 / FFI 绑定（仅 Windows） | 1 | 无 | M（约 350 行） | **低**：qt6 无同名文件，纯新增；仅 `Cargo.lock` 会动 |
| **P2** | 根 Cargo 接线与版本 | 108a129 的 `Cargo.toml` 片段（workspace/feature/optional dep）+ 77e1064 版本号 + `Cargo.lock` | 构建/元数据 | 2 | P1 | S（约 15 行 diff） | **高**：qt6 是 workspace+ffi、无 gui feature、版本 0.2.0-qt6；`Cargo.lock` 不可合并（qt6 已删 eframe 依赖树 4284 行） |
| **P3** | `src/driver.rs` + lib.rs 驱动接线 | 108a129（driver.rs + lib.rs 的 DriverError/Op/run_driver_op/cli_main）+ b66d3d6 + c9a75d4（签名/日志）+ a38115b + 05bd642 | Rust 核心 + CLI | 3 | P2 | L（约 740 行新文件 + 约 120 行 lib.rs） | **中**：`src/driver.rs` 新文件零冲突；`src/lib.rs` 的 hunk 大多在 qt6 未触碰区，仅 `cli_main` / `Op` 附近需手工对齐 |
| **P4** | Windows 设备名解析 | ccd4447 + b5e78b7 + 78687fa（`src/lib.rs` 全部为私有函数） | Rust 核心 | 4 | 无（建议 P3 后，同文件串行） | M（约 200 行 + 测试） | **高**：qt6 在 `DeviceSummary::display_name` / `scan_devices` / Linux sysfs 回退同一区域有 141 行改动，且函数名不同（见 R1） |
| **P5** | 构建脚本迁入 `.github/scripts/` | 108a129 的 rename/删除 + 77e1064/b219bd5 的脚本改动（`build-linux.sh` `build-macos.sh` `build-windows.ps1` `windows-cross-build-linux.ps1`，删除根 `build-windows.ps1`） | CI / 构建 | 5 | 无（文本独立） | M（约 300 行搬迁+改写） | **高**：qt6 三个根脚本已被改成 CMake/Qt 构建，不能整文件覆盖，只能重写「结构 + qt6 的构建命令」 |
| **P6** | CI 内联 libwdi 静态构建 | 108a129 的 Windows job + ca8b2d7 + 30994bc + bba951e + b4afdb2 + ac7494a + 625f489（构建侧） | CI | 6 | P1、P2、P5 | L（约 100 行 YAML，6 轮调试经验） | **高**：qt6 的 `build.yml` 基于旧的 3 平台 cargo 流程（±63 行改动），且 Windows job 要先编 Rust FFI 再编 C++/Qt，需要重新编排 |
| **P7** | 触发/发布策略 + 版本化产物 | 77e1064 + b219bd5（`build.yml` 触发、`mode`、`AUROKU_BUILD_DEBUG`、glob/版本号；脚本的 debug 分支与版本参数） | CI / 发布 | 7 | P6 | M（约 120 行 YAML/脚本） | **高**：qt6 的 `0318c51` 已改成「push qt6 分支构建」，与 main 的「push 任意分支 debug + dispatch release」冲突，需合并成一套 |
| **P8** | 文档 | 108a129 的 `docs/windows-driver.md`（195 行）+ README/README.zh-CN（+56/−29、+49/−26）+ b219bd5 的 README 更新 | 文档 | 8 | P3/P5/P7（引用路径与行为） | M（约 320 行） | **中**：新文档文件零冲突；README 在 qt6 被大改（−208 行），只能重写相关段落 |
| **P9** | `auroku-ffi` 扩展 | 新增 C ABI（见 5.2） | **FFI 暴露** | 与 P10 并行（P3/P4 之后） | P3、P4 | M（约 150-200 行 Rust） | **中**：qt6 的 ffi 是独立新文件，新增函数零冲突；但要改 `FFI_ABI_VERSION` 与错误 kind 约定 |
| **P10** | C++/Qt6 GUI 重写驱动 UI | 108a129（app.rs/worker.rs/i18n.rs）+ c9a75d4 + ba75b4c 的**语义** | **C++ GUI 重写** | 与 P9 并行，最后 | P9 | L（约 600-900 行 C++/Qt） | **低**（文本）：egui 源码在 qt6 已删除；**高**（语义/验收）：需在 `gui/src/ui/pages/` 与 `MainWindow` 里重建状态机 |

### 5.2 P9（FFI）具体契约建议

qt6 现有 ABI：`auroku_ffi_version / auroku_scan_devices / auroku_run_task / auroku_string_free`（qt6 `crates/auroku-ffi/src/lib.rs:345-449`），信封 `{ok,data} / {ok:false,kind,message}`。建议新增：

1. `auroku_driver_install(vid: u16, pid: u16, restore_on_exit: bool) -> *mut c_char` —— 返回 `{ok,data:{vid,pid,restore_on_exit,state_file}}` 或 `{ok:false,kind:"DriverError",message}`；
2. `auroku_driver_restore(vid: u16, pid: u16) -> *mut c_char`；
3. `auroku_driver_pending_restore() -> *mut c_char` —— 无待恢复时返回 `{ok:true,data:null}`；
4. `auroku_driver_log_path() -> *mut c_char` —— GUI 用来在控制台显示 `driver.log` 位置（对应 `app.rs:645-647`）；
5. 可选：`auroku_driver_install(...)` 复用现有 `AurokuLogCallback` 把 `driver::DriverLog` 的进度行流给 C++（qt6 ffi `:44` 已有回调类型；main 的 `DriverLog` 与 `LogSink` 类型不同，桥接时转一层 `Arc::new(move |line| ...)` 即可）。
6. `FFI_ABI_VERSION` 从 `"1"` 升到 `"2"`（qt6 ffi `:37`）；C++ `ApiError` 需把 `"DriverError"` 当成「可恢复」错误类型。
7. **不要**把 `driver::DriverRecord` 直接暴露成 C struct（qt6 ffi 的设计原则是「JSON 信封，不让 C++ 建模 Rust 类型」，见 qt6 ffi 文件头注释 :1-22）。

### 5.3 P10（C++ GUI）交互规格（从 egui 侧逐条抽取）

| # | egui 行为 | main 证据 | C++ 侧等价要求 |
|---|---|---|---|
| 1 | 任务失败且 kind==DriverError → 记录目标并显示驱动卡片 | `app.rs:450-455` | `runTask` 错误回调里判断 kind，保存 last target，切到「驱动待安装」状态 |
| 2 | 卡片：说明文案 + 「安装 WinUSB 驱动」+「恢复原驱动」+ 「退出时自动恢复」复选框（默认关） | `app.rs:1311-1360` | 结果页/概览页底部 action row |
| 3 | 安装成功后自动重试之前失败的任务 | `app.rs:479-481、:626-627` | 保存 pending task+label，安装成功回调里重新发起 |
| 4 | 启动时 pending_restore → 日志提示「上次会话未恢复…」 | `app.rs:291-299` | 启动时查询 FFI #3，写控制台并置卡片 |
| 5 | 安装/恢复前把 driver.log 路径写进控制台 | `app.rs:645-647、:661-663` | 调 FFI #4 显示路径 |
| 6 | 退出时（仅 opt-in）自动恢复原驱动 | `app.rs:2786-2806` | `MainWindow::closeEvent` 里 query pending → restore |
| 7 | 发布版无控制台 + gui.log | `src/bin/auroku-gui/main.rs:5`、`src/bin/auroku-gui/diag.rs:1-35` | CMake `WIN32_EXECUTABLE` / `/SUBSYSTEM:WINDOWS`；等价日志文件或 Qt 日志 |
| 8 | i18n 文案（约 15 个新键） | `src/bin/auroku-gui/i18n.rs`（+27 行，均在 Windows driver 段） | qt6 `gui/src/ui/I18n.cpp` 加同义中文/英文条目 |
| 9 | 驱动进度行（`DriverLog`）当前 GUI 传 `None`，只有 driver.log | `src/bin/auroku-gui/worker.rs:160-183` | 可选增强：把 FFI #5 的日志接进 ConsolePage |

> 说明：P10 的 `windows_subsystem`（Rust 属性）在 C++ 侧是链接器/CMake 设置，不是可移植代码；`diag.rs` 的 `gui.log` 是 egui 专用文件，C++ 侧不需要照抄文件格式，但要有等价的可诊断输出。

### 5.4 建议的落地批次（供 00 总计划参考）

- **批次 1（纯新增/低风险）**：P1 → P2（先只加 crate + feature，不动 qt6 的 C++ 构建）→ P3；此时 CLI 已可 `--op install-driver`。
- **批次 2（Rust 语义合并）**：P4（与 qt6 的 `display_name`/sysfs 实现做**手工三方合并**：保留 qt6 命名与显示格式，叠加 main 的 Windows 注册表回退）→ P8 文档。
- **批次 3（FFI + GUI）**：P9 → P10，可与批次 2 并行开始 P9。
- **批次 4（CI，最后做）**：P5 → P6 → P7。原因是 CI 依赖「qt6 的 CMake/Qt 构建 + main 的 libwdi 静态库 + 新触发策略」三方合并，且 CI 改动无法在本地验证（只能靠 push 试跑）。

---

## 6. 数据核对（可复现）

### 6.1 区间 diff（关键数字）

| 区间 | 文件数 | 插入 | 删除 |
|---|---|---|---|
| `v0.2.0..v0.3.1` | 23 | **2297** | **169** |
| `v0.2.0..b219bd5` | 23 | **2357** | **203** |
| `v0.3.1..b219bd5` | 4（CI×2、README×2） | 74 | 48 |
| `v0.1.0..v0.2.0`（基线，§4） | 15 | 982 | 405 |

新增/关键文件行数（`git diff --numstat v0.2.0..b219bd5`）：
`src/driver.rs` **+719/−0（新文件）**；`src/lib.rs` +364/−4；`crates/auroku-driver-sys/src/lib.rs` +305、`build.rs` +31、`Cargo.toml` +12；`.github/scripts/build-windows.ps1` +180（新）；`.github/workflows/build.yml` +101/−30；`docs/windows-driver.md` +195（新）；`src/bin/auroku-gui/app.rs` +159/−1、`diag.rs` +35（新）、`i18n.rs` +27、`worker.rs` +45/−2、`main.rs` +5、`permissions.rs` +4/−5、`fonts.rs` +2/−2。

### 6.2 `Cargo.toml`（HEAD b219bd5）核实

- `version = "0.3.1"`（:3），`rust-version = "1.88"`（:5），`edition 2021`；
- features：`gui = ["dep:eframe"]`（:13）、`static-libwdi = ["dep:auroku-driver-sys"]`（:16），**default = []**（:10）；
- `[[bin]] auroku`（:18-20）与 `[[bin]] auroku-gui path = "src/bin/auroku-gui/main.rs" required-features = ["gui"]`（:22-25，本次区间未改）；
- 依赖：`eframe 0.33 optional`（:29）、`rusb 0.9 vendored`（:32）、`serde_json preserve_order`（:33）；
- `[target.'cfg(windows)'.dependencies] auroku-driver-sys = { path = "crates/auroku-driver-sys", optional = true }`（:35-37）；
- `[target.'cfg(target_os = "linux")'.dependencies] rustix 1 process`（:39-41）；
- `[workspace] members = ["crates/auroku-driver-sys"]`（:43-44）；
- `[profile.release] lto = true, strip = true`（:46-48）。

`Cargo.lock`：`auroku` 0.2.0→0.3.1，新增 `auroku-driver-sys 0.1.0`（+6/−1）。**qt6 的 lock 已删掉整个 eframe 依赖树（4284 行变更），不能复用 main 的 lock。**

### 6.3 重命名 / 删除 / 权限位（`git diff --summary v0.2.0..b219bd5`）

- `build-linux.sh => .github/scripts/build-linux.sh`（86% 相似，**mode 100755 → 100644**）；
- `build-macos.sh => .github/scripts/build-macos.sh`（75%，mode 100755 → 100644）；
- `windows-cross-build-linux.ps1 => .github/scripts/windows-cross-build-linux.ps1`（95%）；
- 新建 `.github/scripts/build-windows.ps1`；删除根 `build-windows.ps1`（−82）；
- 新文件：`crates/auroku-driver-sys/{Cargo.toml,build.rs,src/lib.rs}`、`docs/windows-driver.md`、`src/driver.rs`、`src/bin/auroku-gui/diag.rs`。
- 注：Linux 脚本在 CI 里显式 `chmod +x`（`build.yml:167`），否则 644 位会 Permission denied。

### 6.4 数量核对

- 提交数：19（v0.2.0..b219bd5）、4（v0.1.0..v0.2.0）、23（777c2d2..b219bd5）；**任务书「22」无法复现**。
- 提交时间跨度：2026-11-04 01:27 → 06:10（+0800），共 19 个提交、约 4 小时 43 分，属于一次集中开发。
- 逐提交合计 +2450/−296 vs 净 diff +2357/−203：差 93 插入/93 删除，正好等于「同一文件多轮改写」被覆盖的部分（已人工核对 `src/driver.rs` 552→719、`build-windows.ps1` 164→180 的演化）。

---

## 7. 风险与未决问题

| # | 风险/未决 | 等级 | 证据与建议 |
|---|---|---|---|
| R1 | **`src/lib.rs` 三方合并冲突**，尤其 `DeviceSummary::display_name` / `scan_devices` / Linux sysfs 回退 | **高** | main 在 v0.2.0 有 `fill_strings_from_sysfs`（298f8d8），qt6 有 `read_sysfs_usb_descriptors` + `device_location` 改名（qt6 lib.rs +141/−25）。建议：**不要整文件取一边**；以 qt6 的命名/显示格式为骨架，把 main 的 Windows 注册表回退（`decode_device_text/windows_device_strings/powershell_device_strings`）作为新增函数叠加；合并后跑 `cargo test` + 手工 check `display_name`。 |
| R2 | **`Cargo.toml`/workspace 结构冲突** | **高** | main：根包 + `members=["crates/auroku-driver-sys"]` + `gui/static-libwdi` feature + `eframe`；qt6：`[workspace] members=[".","crates/auroku-ffi"] resolver="2"` + 无 gui feature + 无 eframe。合并建议：members 三者都留；保留 qt6 的 `auroku-ffi` 与 C++ 构建；`static-libwdi` 仅 `cfg(windows)` 生效；不要恢复 eframe/[[bin]] auroku-gui（qt6 已用 C++ 前端）。 |
| R3 | **`Cargo.lock` 冲突/污染** | 中 | 区间只 +6/−1，但 qt6 侧删了 4284 行（eframe 树）。**不要 cherry-pick lock**；合并 `Cargo.toml` 后重新生成（`cargo generate-lockfile` 或 `cargo update --workspace`），CI 用 `--locked` 验证。 |
| R4 | **`.github/scripts` 重排 vs qt6 根脚本** | **高** | main 删根 `build-windows.ps1`、搬 3 个脚本；qt6 的 3 个根脚本已被改成 CMake/Qt 构建（`build-windows.ps1` 214 行改动、`build-linux.sh` 94 行）。不能覆盖，只能「main 的目录结构/CI-only 理念 + qt6 的构建命令」。迁移后 README 里所有 `./build-*.sh` 引用都要改。 |
| R5 | **CI 里内联构建 libwdi 的易腐性** | 中 | `build.yml` 对 libwdi v1.5.1 的工程名/平台（`detect_64build/installer_x64/embedder/libwdi_static`）和 WDK 8.0 解包布局硬编码，连修 6 轮（ca8b2d7..ac7494a）。qt6 若换 MSVC/CMake 生成器，这段要重新验证；建议按 03 文档的「libwdi 解耦分支」方案改为引用预编译产物。 |
| R6 | **发布策略/触发冲突** | **高** | qt6 `0318c51` 是「push qt6 分支构建」；main `b219bd5` 是「push 任意分支 debug + dispatch release」。合并后必须二选一或组合（例如：push→debug artifact；dispatch→release；分支过滤按需）。注意 `b219bd5` 在 v0.3.1 之后，容易被忘记。 |
| R7 | **版本号语义** | 中 | main 0.3.1 vs qt6 `0.2.0-qt6`（semver 预发布，比较上 < 0.3.1）。若直接取 0.3.1，`auroku_ffi_version` 会显示 `0.2.0-qt6` 的旧字符串（qt6 ffi 用 `CARGO_PKG_VERSION`）。建议汇入后统一为 `0.3.1-qt6` 或 `0.4.0-qt6`，并同步 `Cargo.lock`、README、包名。 |
| R8 | **提权 helper 依赖同目录 `auroku.exe`** | **高（未决）** | `elevate_self` 固定找同目录 `auroku.exe`，注释明说 `auroku-gui does not parse --op`（`src/driver.rs:477-495`）。如果 qt6 发布包只带 GUI（需 02 文档确认），则 P10 的「安装」按钮会失败；方案：发布包同时带 CLI，或给 C++ 侧/FFI 提供 `--op` 等价入口。 |
| R9 | **LGPLv3 合规** | 中 | 静态链接 libwdi 的发布包必须带 `LICENSE-LGPL-3.0.txt`（`build-windows.ps1:163-171`、`docs/windows-driver.md:193-195`）。qt6 的 CMake 打包/CI 若不经过该脚本，会静默丢失许可证 → 验收项。 |
| R10 | **未决：qt6 的 C++ 侧是否持有 vid/pid** | 中 | `auroku_run_task` 的错误信封不带 vid/pid（qt6 ffi :98-108），驱动卡片需要当前设备目标；C++ `AppState` 应已保存选择项，但需 02 文档确认；否则 FFI 需在 `DriverError` 里附带 vid/pid。 |
| R11 | **未决：qt6 是否有 driver.log 展示位** | 低 | qt6 有 `ConsolePage`；P10 可选把驱动进度流进去，否则用户只能去 `%LOCALAPPDATA%/auroku/driver.log` 找日志（egui 也只显示路径）。 |
| R12 | **main 侧小缺陷（移植时顺手修）** | 低 | `src/driver.rs:360-367` restore stub 文案写成 "installation"；`src/driver.rs:652` 错误串有多余空格；`docs/windows-driver.md:71-72` 与 `:74-75` 重复段落；`docs/windows-driver.md:67-69` 与 README 的「需要 wdi-simple.exe」与「发布版内置 libwdi」两种口径并存。 |
| R13 | **未决：`DriverLog` vs `LogSink` 类型重复** | 低 | `driver::DriverLog`（`src/driver.rs:224`）与 `lib::LogSink`（`src/lib.rs:374`）结构相同、类型不同；FFI 桥接若要复用 `AurokuLogCallback`，建议在 P9 里统一成一个别名（注意别破坏 qt6 现有签名）。 |

---

## 附录 A：本报告使用的只读命令（可复现）

```powershell
git -C C:/Users/Administrator/Documents/proj/aurokku-rs rev-list --count v0.2.0..b219bd5
git -C ... log --reverse --date=short --pretty=format:'%h %ad %s' v0.2.0..b219bd5
git -C ... log --reverse --format='COMMIT %h %s' --numstat v0.2.0..b219bd5
git -C ... log --reverse --format='### %h|%ad|%s' --shortstat v0.2.0..b219bd5
git -C ... diff --stat v0.2.0..v0.3.1
git -C ... diff --stat v0.2.0..b219bd5
git -C ... diff --stat v0.3.1..b219bd5
git -C ... diff --numstat v0.2.0..b219bd5 -- src/lib.rs src/driver.rs src/bin/auroku-gui Cargo.toml Cargo.lock
git -C ... diff --numstat v0.2.0..b219bd5 -- .github docs README.md README.zh-CN.md build-windows.ps1 build-linux.sh build-macos.sh windows-cross-build-linux.ps1
git -C ... diff --summary v0.2.0..b219bd5
git -C ... diff -U0 v0.2.0..b219bd5 -- src/lib.rs
git -C ... log --reverse -p --format='COMMIT %h %s' v0.2.0..b219bd5 -- .github/workflows/build.yml
git -C ... log -S 'pending_restore' --oneline v0.2.0..b219bd5 -- src/lib.rs src/bin/auroku-gui
git -C ... show --stat --format='%h %s' 298f8d8 83ef874 3a81f4b b9e51cc 77e1064 b219bd5
git -C ... show-ref --tags
# qt6 只读核对（细节以 02 文档为准）
git -C C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6 log --reverse --format='%h %ci %s' 777c2d2..HEAD
git -C .../qt6 diff --stat 777c2d2..HEAD
git -C .../qt6 show HEAD:Cargo.toml
git -C .../qt6 diff 777c2d2..HEAD -- src/lib.rs
```

## 附录 B：qt6 侧只读事实速查（供 02/00 文档交叉引用）

- qt6 HEAD 51ff33b（10-31 05:01），相对 777c2d2 共 10 个提交，69 文件 +7730/−9184。
- 删除了整个 `src/bin/auroku-gui/`（app.rs −2583、elevate.rs −271、fonts −97、i18n −488、main −84、theme −267、widgets −873、worker −171），新增 `gui/`（CMake + C++，`gui/src/ui/pages/` 7 个页面（Batch/Console/Overview/Pin/Probe/Session/Uim））与 `crates/auroku-ffi/`（+508 行）。
- 保留根构建脚本（`build-windows.ps1` 214 行改动、`build-linux.sh` 94 行、`build-macos.sh` 84 行）；无 `.github/scripts/`、无 `docs/`。
- `Cargo.toml`：`[workspace] members = [".", "crates/auroku-ffi"] resolver = "2"`，版本 `0.2.0-qt6`，无 `gui`/`eframe`/`static-libwdi`。
- `src/lib.rs` +141/−25：`usb_location`→`device_location`、`read_sysfs_usb_descriptors`、`display_name` 改「制造商 — 产品」。
- `crates/auroku-ffi/src/lib.rs`：4 个 extern C（:345-449），`FFI_ABI_VERSION = "1"`（:37），JSON 信封 `{ok,data}/{ok:false,kind,message}`。
- 版本标签 `v0.2.0-qt6` 是附注标签 515ea6e → 提交 e21f816（10-30 23:05），其父为 `73d3e13`；51ff33b 在标签之后。

> 完。本文档只描述 main 侧；qt6 侧的精确冲突量化、FFI/C++ 改动清单、三种汇入策略见 `docs/plan/02-qt6-gap-analysis.md`，libwdi 静态库解耦方案见 `docs/plan/03-libwdi-decoupling.md`。
