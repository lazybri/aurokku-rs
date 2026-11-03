# 02 · qt6 分支现状、与 main 的功能差距及汇入策略

> 角色：**只读分析**（qt6-analyst）。本文所有 git 操作均为只读（`log/show/diff/merge-tree`），另有一个 `%TEMP%` 下的 `git clone --shared` 试合并实验（已删除）。
> 基线：main HEAD `b219bd5`（`v0.3.1` = `77e1064`），qt6 HEAD `51ff33b`（`v0.2.0-qt6` = 附注标签 `515ea6e` → 提交 `e21f816`），共同祖先 `777c2d2`（`v0.1.0` / `v0.1.0-qt6`）。
> 数据采集：git 2.54.0.windows.1 / cargo 1.99.1 / Windows + PowerShell；路径一律正斜杠。

---

## 0. TL;DR（12 条）

1. **两条分支是同一祖先的两次独立重写**：qt6 自 `777c2d2` 起 10 个提交、69 文件 `+7730/−9184`；main 自 `777c2d2` 起 23 个提交、25 文件 `+3318/−587`。两者各自实现了同一批后端能力（ACL、sysfs 设备名、bus/address 钉设备），实现重复但不共享代码。
2. **最大结构性冲突是「qt6 删除 egui GUI / main 在同一批文件上继续开发」**：`src/bin/auroku-gui/{app,fonts,i18n,main,worker}.rs` 在 qt6 被删、在 main 被改 → 5 个 `modify/delete` 冲突；另加 `build-windows.ps1`（main 重命名到 `.github/scripts/`、qt6 改内容）共 **6 个删改冲突**。
3. **内容冲突 8 文件 / 37 块**：`src/lib.rs`(9)、`.github/workflows/build.yml`(8)、`README.md`(6)、`README.zh-CN.md`(5)、`.github/scripts/build-macos.sh`(4)、`Cargo.toml`(2)、`Cargo.lock`(2)、`.github/scripts/build-linux.sh`(1)。合计 **14 条未合并路径 / 43 个冲突单元**。
4. **`src/lib.rs` 的 9 块冲突里 6 块是「同一功能两套命名」**：main 用 `usb_location` + `set_usb_location()`（`main:src/lib.rs:384,520,587,782`），qt6 用 `device_location` 构造参数（`qt6:src/lib.rs:346,482,511,739`）。语义完全相同（都在 `open_device` 里用 `is_none_or` 按 bus/address 过滤候选设备）。
5. **另外 3 块是「两套 sysfs 实现」**：main `fill_strings_from_sysfs`（`main:src/lib.rs:1899`）；qt6 `read_sysfs_usb_descriptors`（`qt6:src/lib.rs:1859`）。main 那套更全（还带 Windows 注册表解码）。
6. **`Cargo.toml` 会静默产生非法清单**：合并结果同时存在两个 `[workspace]` 表（第 1 行 qt6 的、第 48 行 main 的），且**没有冲突标记**；实测 `cargo metadata` 直接 `error: duplicate key`（exit 101）。同时 main 的 `[[bin]] auroku-gui` 与 `eframe` 依赖被**静默丢弃**（方向恰好正确，但必须显式确认，不能当成「合并已解决」）。
7. **`src/lib.rs` 合并后必然编译失败**：合并结果 `run_task` 里同时出现 `config.device_location`（qt6）和 `device.set_usb_location(config.usb_location)`（main）——两边命名各留一半，属「无冲突标记的逻辑冲突」。
8. **`build-windows.ps1` 的重命名没被识别**：合并后根目录留着 qt6 版，同时新增 main 的 `.github/scripts/build-windows.ps1`（egui 版），仓库里两份 Windows 脚本并存，CI 写错路径就构建出 egui 版。
9. **main 的 v0.3.x 主力新功能 qt6 完全没有**：`src/driver.rs`(719 行)、`crates/auroku-driver-sys`(305+31+12)、`docs/windows-driver.md`(195 行)、`DriverError` 错误类型、`--op install-driver/restore-driver` CLI、`driver.log`/`gui.log`、非 ASCII 设备名注册表解码。这些合并时作为**新增文件**自动落地，风险低。
10. **qt6 有 main 完全没有的东西**：Qt6/C++ GUI（`gui/` 45 文件）、`crates/auroku-ffi` C ABI、`--smoke-test`（main 的 egui 版与 CI 都没有）、三平台 GUI smoke test、静态 Qt/Wayland 处理、`SECURITY_AUDIT.md`、`.vscode/settings.json`。
11. **推荐策略 A（merge）**：从 qt6 拉 `qt6-integrate-0.3.1` → `git merge --no-ff --no-commit b219bd5` → 按「保留 qt6 GUI / 取 main 的 Rust 后端 / 删除 egui / 重写 Cargo.toml 与 CI」四类策略一次解决 → 编译修复 → 提交 → 再单独提交 FFI+Qt UI 新功能。cherry-pick（策略 B）只有 9/23 个提交能干净落地，diff-patch（策略 C）只适合 driver 栈，均不建议作为主路径。
12. **汇入后要补 4 个 extern C 接口**（`auroku_driver_install/restore/status` + `auroku_state_dir`）+ ABI 升到 2；Qt 侧需新增「WinUSB 驱动」卡片、`gui.log`、`DriverError` 分支与安装后自动重试。`RustBridge/RustWorker` 的单工作线程模型**够用**，但现有 `m_busy` 静默丢请求的写法必须先改。

---

## 1. 事实基线与阅读方式

| 项 | main | qt6 |
|---|---|---|
| 工作树 | `C:/Users/Administrator/Documents/proj/aurokku-rs` | `C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6` |
| HEAD | `b219bd5` | `51ff33b` |
| 标签 | `v0.1.0`=`777c2d2`、`v0.2.0`=`b9e51cc`、`v0.3.1`=`77e1064` | `v0.1.0-qt6`=`777c2d2`、`v0.2.0-qt6`=附注标签 `515ea6e`→`e21f816` |
| 自祖先起提交数 | 23（`3a81f4b`…`b219bd5`） | 10（`6b498be`…`51ff33b`） |
| 自祖先起 diffstat | 25 文件 `+3318 −587` | 69 文件 `+7730 −9184` |

main 的 23 个提交按主题分四段（`git log --oneline 777c2d2..main`）：

1. v0.2.0：`3a81f4b`(per-device USB ACL 生命周期)、`83ef874`(审计/clippy)、`298f8d8`(免 root 设备名)、`b9e51cc`(版本 0.2.0)；
2. 静态 libwdi：`108a129` 起 9 个 `feat/fix/ci(windows)`；
3. 设备名与日志：`ccd4447`、`b5e78b7`、`78687fa`、`c9a75d4`、`ba75b4c`；
4. 发布工程：`77e1064`、`b219bd5`。

qt6 的 10 个提交：`6b498be`(Qt6 GUI 重写，一次性 68 文件)、`2cb653b`(Ninja BYPRODUCTS)、`67ca821`(post-build 引号)、`519b1a6`(FFI 静态链接进 GUI)、`0318c51`(CI 加 qt6 分支)、`8c30cfd`(per-device USB ACL，C++ 版)、`546a1c4`(ignore vscode)、`73d3e13`(USB 发现 + polkit 等待)、`e21f816`(版本 0.2.0-qt6)、`51ff33b`(Wayland 默认)。

> **映射关系（重要）**：main 的 `3a81f4b`(ACL 生命周期) ≈ qt6 的 `8c30cfd`(C++ UsbAccess)；main 的 `298f8d8`(sysfs 设备名) ≈ qt6 `73d3e13` 里的 `read_sysfs_usb_descriptors`。两组是**独立实现**，不是共享代码，合并时只能二选一或并存（并存会导致重复调用）。

---

## 2. qt6 架构地图

### 2.1 体量与布局

```
qt6 (51ff33b)
├─ Cargo.toml                     [workspace] members = [".", "crates/auroku-ffi"]
├─ src/{lib.rs,main.rs}           lib.rs 2769 行（符号表见 §2.2）
├─ crates/auroku-ffi/             C ABI 桥：Cargo.toml + src/lib.rs(509)
├─ gui/                           45 文件：CMakeLists.txt(187) + README(112) + src/
│   ├─ src/main.cpp(104)          CLI 参数、QPA、主题/i18n 初始化
│   ├─ src/core/                  AppState(135+93)、PageHost(38)、RustBridge(65+48)、
│   │                             RustWorker(85+33)、JsonHelpers(54+15)、FfiApi.h(16)
│   ├─ src/platform/UsbAccess     Linux ACL：cpp(320)+h(33)
│   └─ src/ui/                    MainWindow(820+88)、Page(207+45)、Theme(253+72)、
│                                 Widgets(722+130)、I18n(395+37)、Fonts(88+19)、
│                                 Icons(259+39)、ConfirmDialog(58+11)、pages/×7
├─ build-windows.ps1(258) / build-linux.sh(238) / build-macos.sh(224)
├─ windows-cross-build-linux.ps1
├─ .vscode/settings.json          cmake.sourceDirectory（含绝对路径，见 §7）
├─ .github/workflows/build.yml(326)
└─ SECURITY_AUDIT.md(54)
```

### 2.2 `src/lib.rs` 函数级差异

**qt6 相对 `777c2d2` 只改了 141 行**（`git -C <qt6> diff --stat 777c2d2..qt6 -- src/lib.rs` = 141），可完整枚举，主题只有两个：「把操作钉到具体 USB 设备节点」和「设备名拼接 / sysfs 回退」：

| # | 位置（qt6 行号） | 变更 | main 侧对应实现 |
|---|---|---|---|
| 1 | `DeviceConfig` 346 / `new` 361 | 新增 `pub device_location: Option<(u8,u8)>` | main 同字段但叫 `usb_location`（`main:src/lib.rs:384`） |
| 2 | `AuRokkuDevice` 482 / `new` 511 | 新增 `device_location` 字段 + **构造函数第 3 个参数** | main 无构造参数，改用私有 `set_usb_location()`（`main:src/lib.rs:587`） |
| 3 | `open_device` 727–760 | 枚举候选时加 `&& self.device_location.is_none_or(...)`，按 bus/address 过滤 | 同逻辑但用 `usb_location`（`main:src/lib.rs:782`） |
| 4 | `display_name` 1837–1857 | 重写为 `manufacturer — product`（厂商名与产品名忽略大小写相同时只显示一个），缺项回退 | main 未改此函数（合并后 qt6 版会保留） |
| 5 | 新增 `read_sysfs_usb_descriptors` 1859–1901 | Linux sysfs 按 busnum/devnum 匹配，读 manufacturer/product/serial | main 的 `fill_strings_from_sysfs`（`main:src/lib.rs:1899`），功能重叠 |
| 6 | `scan_devices` 1903–1980 | 在 `out.push` 前补 sysfs 描述符 | main 同名位置（`main:src/lib.rs:2107-2130`）+ Windows 注册表回退 |
| 7 | `run_task` 1981 | 透传 `config.device_location` 到构造函数 | main 走 `device.set_usb_location(config.usb_location)`（`main:src/lib.rs:2190`） |
| 8 | `cli_main` 2503 | `DeviceConfig { device_location: None, .. }` | 同 |
| 9 | tests 2588+ | 新增 2 个测试：`device_display_name_includes_manufacturer_and_product`(2748)、`sysfs_descriptors_match_bus_and_address`(2763) | main 对应测试在 `main:src/lib.rs:2906+` |

**三方能力对照（函数级结论）**

| 能力 | `v0.2.0`(`b9e51cc`) | main HEAD(`b219bd5`) | qt6(`51ff33b`) |
|---|---|---|---|
| bus/address 精确选设备 | 有（`3a81f4b`，`usb_location`+setter） | 有 | 有，但叫 `device_location`+构造参数 |
| Linux sysfs 设备名 | 有 `fill_strings_from_sysfs`（`298f8d8`） | 有 | 有，另一套 `read_sysfs_usb_descriptors`（`73d3e13`） |
| Windows 注册表设备名（VID:PID 解析 + 非 ASCII 解码） | 无 | 有：`windows_device_strings`/`decode_device_text`/`powershell_device_strings`（`ccd4447`,`78687fa`） | **无** |
| `pub mod driver`（安装/恢复 WinUSB） | 无 | 有：`main:src/lib.rs:24` + `src/driver.rs`(719) | **无** |
| `AuRokkuError::Driver`→`"DriverError"` | 无 | 有：`main:src/lib.rs:42-73` + `open_failure`(85) | **无** |
| `--op install-driver/restore-driver`、`--restore-on-exit`、`--driver-log-append` | 无 | 有：`main:src/lib.rs:2267,2290,2363,2748` | **无** |
| `scan_devices` 名字缓存（`NameCache`） | 无 | 有：`main:src/lib.rs:2107` | **无** |

> 结论：`src/lib.rs` 不是「两个版本对等分叉」，而是 **main 在深度扩展、qt6 只做窄改**。因此 lib.rs 冲突应按「**以 main 为底，把 qt6 的窄改叠加回去**」处理，而不是反过来。

### 2.3 `crates/auroku-ffi`：C ABI 表面

- **4 个导出函数**（`crates/auroku-ffi/src/lib.rs`）：`auroku_ffi_version()`(346)、`auroku_scan_devices(vid:i32,pid:i32)`(360)、`auroku_run_task(request_json, cb, user_data)`(407，unsafe)、`auroku_string_free(char*)`(444)。ABI 版本常量 `FFI_ABI_VERSION = "1"`(37)，回调类型 `AurokuLogCallback`(44)。
- **JSON 信封协议**：成功 `{"ok":true,"data":...}`；失败 `{"ok":false,"kind":"<Value/Runtime/Timeout/Panic>","message":"..."}`。`kind` 直接取自 `AuRokkuError::kind()`（`ApiError::from` 是通用实现），所以 main 新增 `DriverError` 后**不改 FFI 代码即可透出**。
- **所有权/释放约定**：所有返回字符串由 Rust `CString::into_raw` 分配，必须 `auroku_string_free`。C++ 侧 `RustWorker::parseEnvelope` 先 `QByteArray bytes(raw)` 拷贝、再立刻 `auroku_string_free(raw)`（`gui/src/core/RustWorker.cpp:29-56`），解析失败路径也不泄漏。
- **回调约定**：日志回调只在 `auroku_run_task` 调用期间、在调用线程上被调用，`line` 指针仅当次有效，实现必须拷贝；Rust 侧用 `unsafe impl Send/Sync for LogBridge` 并注释该不变量；C++ 用 `logTrampoline`（`RustWorker.cpp:77`）转发。
- **panic 安全**：`guarded()` 全部 `catch_unwind`，panic → `{"ok":false,"kind":"Panic"}`；消息含内嵌 NUL 时回退为固定 JSON。
- **请求 schema**（`build_config`）：`vid`/`pid` 必填 u16；`bus`+`address` 必须成对且 >0，映射到 `config.device_location`；`sn/interface/ep_out/ep_in/verbose/type2` 可选；另有 `task` 对象。
- **task.kind 全集**（与 `Task` 枚举一一对应）：`status`、`readuimlk`、`clearlk`、`writelk`(value=8 hex，control 默认 1)、`readiccid`、`readpin`、`resetpin`、`maintenance_bit`(control 默认 0)、`batch`(profile=basic|uimlk)、`probe`(probe=list|seri|scdp，timeout_secs 默认 1.0)、`exituimlk`、`exit`。
- 自带 5 个单测，其中 `config_accepts_selected_usb_location`、`config_rejects_incomplete_usb_location` 直接锚定 `device_location` 命名（改名必须同步改测试）。

### 2.4 `gui/`：CMake 构建与线程模型

**CMake（`gui/CMakeLists.txt`，187 行）**

- `find_package(Qt6 6.5 REQUIRED COMPONENTS Widgets)`(17)，C++20，`CMAKE_EXPORT_COMPILE_COMMANDS ON`。
- Rust 后端：`add_custom_target(auroku_ffi_build ALL ...)`(59) 执行 `cargo build --locked -p auroku-ffi`（Debug→debug profile，其余→`--release`）；`BYPRODUCTS`(64) 声明产物避免 Ninja clean 失败（`2cb653b`）；`add_library(auroku_ffi STATIC IMPORTED GLOBAL)`(69) —— `519b1a6` 之后是**静态链接**，运行时不需要 `.so/.dll`。
- **不引入 libudev**：Linux 上给 cargo 注入 `PKG_CONFIG=/bin/false`（`AUROKU_FFI_ENV`）；CMake 配置阶段再显式传系统 `pkg-config`（该踩坑记录在 `SECURITY_AUDIT.md` 的「Qt / WrapDBus1 构建诊断」）。
- 可执行：`qt_add_executable(auroku-qt WIN32 MACOSX_BUNDLE ...)`(123)，`OUTPUT_NAME "auroku-gui"`(137)，版本宏 `AUROKU_APP_VERSION`（= 工程版本 + 后缀 `-qt6`）。
- 静态 Qt 支持：仅当 `Qt6::QWaylandIntegrationPlugin` 是 `STATIC_LIBRARY` 时 `qt_import_plugins(... INCLUDE ...)`(132) + 定义 `AUROKU_HAVE_WAYLAND_PLUGIN`（`51ff33b`）。
- 平台链接：Windows `ws2_32 userenv bcrypt ntdll advapi32 ole32 setupapi cfgmgr32 dbghelp user32 hid`；macOS `CoreFoundation/IOKit/Security/iconv/objc`；Linux `m rt util`（Rust std 原生依赖）。
- 部署：Windows 找 `windeployqt`(172)；macOS `MACOSX_BUNDLE_*`(184)。

**线程模型（RustBridge / RustWorker）**

- `RustBridge` 构造时创建 `QThread`（`objectName="auroku-usb"`）并把 `RustWorker` `moveToThread`（`gui/src/core/RustBridge.cpp:11-24`）；析构时 `quit()+wait()`（同文件 34-41）。
- 所有调用经 `QMetaObject::invokeMethod(..., Qt::QueuedConnection)` 投递（47、57），UI 线程永不阻塞在 libusb 超时上。
- `RustWorker` 两个槽：`scan(vid,pid)`、`runTask(request,label)`；三组信号：`scanStarted/scanFinished/scanFailed`、`taskStarted/taskFinished`、`logLine`（`RustWorker.h:13-27`）。
- **串行化**：`RustBridge::runAsync` 在 `m_busy` 时直接 `return`（`RustBridge.cpp:55-58`）——注意是**静默丢弃**，不发错误信号；`scanAsync` 完全不检查 busy。加入「驱动安装（可能等 UAC 数十秒）」后该语义必须改成显式拒绝。
- 结果解码：`parseEnvelope` 返回 `{ok,data,kind,message}`，`describe()` 拼 `"kind: message"`（`RustWorker.cpp:29-56`）。

### 2.5 页面清单（`PageId`，`gui/src/core/PageHost.h:9`）

| # | 页面 | 文件（行数） | 功能 | 触发的 FFI task |
|---|---|---|---|---|
| 0 | Overview | `OverviewPage.cpp`(304) | 设备下拉框 + 扫描全部 / 按 VID:PID 扫描 + 设备表单（vid/pid/sn/interface/ep_out/ep_in/type2/verbose）+ 快捷操作 | `auroku_scan_devices` + 快捷 task |
| 1 | Uim | `UimPage.cpp`(289) | UIM lock 读 / 写 / 清除 + NV 10481 维护位 | `readuimlk/writelk/clearlk/maintenance_bit` |
| 2 | Pin | `PinPage.cpp`(155) | 手机锁码 NV 82 读取/重置 + ICCID record 0x59 | `readpin/resetpin/readiccid` |
| 3 | Probe | `ProbePage.cpp`(275) | USB 布局 / SERI 回显 / SCDP 探测，带超时与分段选择 | `probe`(list/seri/scdp) |
| 4 | Batch | `BatchPage.cpp`(112) | 单会话内批量查询（basic / uimlk profile） | `batch` |
| 5 | Session | `SessionPage.cpp`(128) | 服务退出命令与会话记录 | `exituimlk` |
| 6 | Console | `ConsolePage.cpp`(165) | USB 流量日志（verbose 开关、自动滚动、计数）+ 最近一次操作的完整 JSON | 无（消费 `logLine`/结果） |

宿主逻辑集中在 `MainWindow`（`MainWindow.cpp` 820 行，实现 `PageHost`）：`startTask`(462) 负责表单校验、Linux 上强制「必须先选中设备且 ACL 仍有效」、并给请求注入 `bus`/`address`（462-500 行）；`scanDevices`(505) 与 `selectDevice` 在切换/重扫前先 `revoke` ACL（525、551），选中新设备时 `grant`(567)；`closeEvent`(99-115) 关闭前再 revoke，失败则阻止关闭。

UI 基建：`Theme`（light/dark 调色板 + QSS）、`I18n`（中文源串 → 英文词典，运行时切换）、`Fonts`（CJK 字体发现）、`Icons`（手绘 24×24）、`Widgets`（卡片/横幅/芯片/指标/开关/分段控件）、`Page`（基类 + 结果摘要）、`ConfirmDialog`。`main.cpp` 支持 `--smoke-test`、`--page=N`、`--screenshot=FILE`、`--lang=en|zh`、`--dark/--light`，并支持环境变量 `AUROKU_DUMP_QSS`。

### 2.6 根构建脚本与 CI（qt6 侧）

| 脚本 | 关键行为 |
|---|---|
| `build-windows.ps1`(258) | 参数 `-Target/-QtPrefix/-SkipGui`；`RUSTFLAGS=-C target-feature=+crt-static` → 静态 CRT CLI；`objdump -p` 检查导入表只含 System32 DLL（MinGW/Qt/FFI DLL 泄漏即失败）；自动发现 Qt（`C:/Qt` 两层遍历、`QT_ROOT_DIR`、`Qt6_DIR`、`C:/msys64/mingw64`）并校验 MinGW/MSVC 匹配；拷 Qt DLL + platforms/styles/imageformats/tls/generic/networkinformation 插件；**额外补 `qoffscreen.dll`/`qminimal.dll`** 以支持无桌面 smoke test；MinGW 下对导入表做闭包遍历补齐 ICU/harfbuzz 等；`Compress-Archive` 出 `dist/auroku-windows-<arch>.zip` |
| `build-linux.sh`(238) | musl 静态 CLI：建 `target/musl-uapi` 符号链接解决 musl-gcc 缺 kernel UAPI 头；`PKG_CONFIG=/bin/false`；`-C target-feature=+crt-static -C relocation-model=static`；校验 `file` 为 static、无 `PT_INTERP`、无 `DT_NEEDED`；GUI 用 cmake+ninja（`env -u RUSTFLAGS -u C_INCLUDE_PATH -u PKG_CONFIG` 清理环境），并校验 GUI 不依赖 `libauroku_ffi.so`、不含构建机路径 |
| `build-macos.sh`(224) | 拒绝非 macOS 主机；`--universal`（默认，lipo arm64+x86_64）/`--native`；CLI ad-hoc codesign；GUI 用 cmake 构建**本机架构** bundle、`macdeployqt`、补 `libqoffscreen/libqminimal`、`codesign --deep` |
| `.github/workflows/build.yml`(326) | 触发：push `qt6` 分支、tag `v*`、PR、`workflow_dispatch`（version 必填、prerelease/draft）。`test` job：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets`。`build` 三平台矩阵：windows-latest/msvc `win64_msvc2022_64`、ubuntu-22.04/musl `linux_gcc_64`、macos-14 双 target `clang_64`；`jurplel/install-qt-action@v4` 固定 **Qt 6.8.3**；**三平台都跑 GUI smoke test**（Windows offscreen、Linux offscreen+QT_PLUGIN_PATH/LD_LIBRARY_PATH、macOS offscreen）。`release` job 汇总产物发布 |

---

## 3. 功能对齐矩阵（main `v0.2.0`→`v0.3.1` vs qt6）

判定口径：**Rust**=后端已实现；**FFI**=能通过 C ABI 触达；**C++**=Qt 侧自建实现；**缺失**=qt6 无等价实现。

### 3.1 总表

| # | 功能（main 来源） | main 实现位置 | qt6 是否有等价 | qt6 实现位置 / 缺口 |
|---|---|---|---|---|
| 1 | 设备名解析（Linux sysfs，免 root） | `298f8d8`：`src/lib.rs:1899` `fill_strings_from_sysfs` | 有（不同实现） | Rust：`qt6:src/lib.rs:1859` `read_sysfs_usb_descriptors`；`display_name`(1837) 还更友好 |
| 2 | 设备名解析（Windows VID:PID + 非 ASCII 注册表解码） | `ccd4447`/`78687fa`：`main:src/lib.rs:1943,1964,2044` | **缺失** | 无。qt6 的 sysfs 函数是 `#[cfg(target_os = "linux")]`，Windows 上仍只能读描述符 |
| 3 | USB ACL 生命周期（`3a81f4b`） | `src/bin/auroku-gui/permissions.rs`(509)：pkexec 再执行自身 `--grant-usb-access/--revoke-usb-access`、`PKEXEC_UID` 校验、多设备 `GrantedDevice{acl_state}` 编解码、`refuse_root_execution` | 有（不同实现） | C++：`gui/src/platform/UsbAccess.cpp`(320)。差异：**单设备**、直接 `pkexec setfacl`（不自我 re-exec 做 helper）、`getfacl -cpn` 比对 user/mask、`setfacl -n -m u:UID:rw-`、失败回滚、按 `st_dev/st_ino` 防节点复用、析构里 revoke、polkit 等 120s |
| 4 | USB 发现 + polkit 等待 | main：`permissions.rs:71` `grant_selected_device`（`Command::output()` 阻塞、无超时） | 有且更稳 | C++ `run()` 有 `waitForStarted(10s)` / `waitForFinished(5s/120s)`；`73d3e13` 同时在 lib.rs 引入 bus/address 钉设备 |
| 5 | 驱动安装（WinUSB） | `main:src/driver.rs:293` `install` → `install_backend`(393 static-libwdi / 408 wdi-simple) → `crates/auroku-driver-sys::install_winusb`(175) | **缺失** | 无 driver 模块、无 `--op install-driver`；Windows GUI 遇到 usbser.sys 只能报错 |
| 6 | 驱动恢复（含删 libwdi 驱动包） | `main:src/driver.rs:337` `restore` → `restore_backend`(426)：`pnputil /delete-driver ... /uninstall /force` 清包 + `remove_device`(499) | **缺失** | 同上 |
| 7 | 驱动「删除」（清包 / 删节点） | 合并在 6 内：`driver_packages`(540)、`device_instance_ids`(564)、`remove_device`(499) | **缺失** | 同上 |
| 8 | INF/CAT 生成 | `crates/auroku-driver-sys/src/lib.rs:227`（先 `wdi_create_inf` 再 `wdi_install_driver`(245)） | **缺失** | 同上 |
| 9 | `driver.log` | `main:src/driver.rs:227` `log_path`、249 `write_log_file`、264 `reset_log`（`c9a75d4` 记录安装/恢复全过程） | **缺失** | 无 |
| 10 | `gui.log` | `src/bin/auroku-gui/diag.rs`(35)：`diag::log` 同时写 stderr 与 `state_dir()/gui.log`（启动截断） | **缺失** | Qt 侧只有零星 `std::fprintf(stderr, ...)`（`UsbAccess.cpp` 析构） |
| 11 | `DriverError` 诊断（usbser.sys 提示） | `main:src/lib.rs:42-118` `AuRokkuError::Driver` + `open_failure` | **缺失** | qt6 只有 Value/Runtime/Timeout 三态 |
| 12 | `pending_restore` 崩溃恢复提示 | `main:src/driver.rs:136` + `main:src/lib.rs:2817`（CLI 启动告警）+ `app.rs:291,2790`（GUI 启动/退出） | **缺失** | 无 driver-state.json 概念 |
| 13 | 静默 smoke test（`--smoke-test`） | main **没有**（`git grep smoke-test main` 无命中） | **qt6 独有** | `gui/src/main.cpp:31,120`；CI 三平台都跑 |
| 14 | Windows 子系统 release 构建 | `ba75b4c`：`src/bin/auroku-gui/main.rs` 加 `cfg_attr(all(windows, not(debug_assertions)), windows_subsystem="windows")` | 天然满足 | `qt_add_executable(... WIN32 ...)`（`CMakeLists.txt:123`）——Debug/Release 都是 GUI 子系统 |
| 15 | 调试构建 CI 策略 | `b219bd5`：push 所有分支 → `AUROKU_BUILD_DEBUG=1` 只出 Actions 构件；release 仅 `workflow_dispatch mode=release`；产物带 `-debug` 命名（`77e1064`） | **缺失** | qt6 CI 只有 release 构建，`push.branches=[qt6]` |
| 16 | 静态 CRT 校验 | `.github/scripts/build-windows.ps1` 用 `dumpbin` `Assert-StaticRuntime` 检查 CLI **和 GUI** | 部分 | `build-windows.ps1` 用 `objdump` 只检查 CLI 导入表（未查 GUI、未点名 CRT） |
| 17 | 静态 libwdi 打包 + LGPL 合规 | `108a129`+`ca8b2d7` 等：CI 构 `libwdi.lib`、`static-libwdi` feature、包内附 `libwdi/COPYING-LGPL` | **缺失** | 无 `crates/auroku-driver-sys`，Windows 包内无 libwdi |
| 18 | musl 全静态 CLI | `.github/scripts/build-linux.sh` | 有 | `build-linux.sh` 等价且校验更细（PT_INTERP/DT_NEEDED/ldd） |
| 19 | macOS 通用二进制 | `build-macos.sh` 对 CLI **和 GUI** 都 lipo | 部分 | CLI universal；GUI 仅本机架构（`build-macos.sh` GUI 段无 lipo） |
| 20 | 版本化发布归档 | `77e1064`：`auroku-linux-x86_64-v0.3.1.tar.gz` 等 | **缺失** | 固定文件名，无 debug 区分 |

### 3.2 三项容易误判的差异

- **ACL 是两套实现，不能「二选一丢弃」**：main 的模型是「GUI 用 pkexec 重新执行自己当 root helper」，要求可执行文件路径可信（`trusted_executable`），支持**多设备累积授权**；qt6 的模型是「直接 pkexec 系统 `setfacl`」，只支持**当前单设备**，但多了 polkit 超时、dev/ino 复核、mask 冲突回滚。合并后建议**保留 qt6 的 C++ 实现**（GUI 已是 C++，main 版随 egui 删除而消失），并补两项：**多设备授权**（现切换设备必先 revoke，无法同时保持两台可访问）、**拒绝 root 运行**（main 的 `refuse_root_execution`，可放进 `main.cpp`）。
- **driver 栈可整体搬入，不需要融合**：qt6 没有任何 driver 代码，main 的 `src/driver.rs` / `crates/auroku-driver-sys` / `docs/windows-driver.md` 在试合并里是 `A`（新增）状态，唯一工作是接到 FFI 与 Qt UI（§6）。
- **CI 是「政策差异」不是「代码差异」**：`build.yml` 必然 8 处冲突，正确做法不是逐块挑，而是**按新政策重写**（Qt 三平台矩阵 + main 的 debug/release 分流 + libwdi 构建 + 版本化归档 + qt6 的 smoke test）。

---

## 4. 试合并量化

### 4.1 三次 `merge-tree`（只读，不落地）

```bash
git -C <main> merge-tree --write-tree --name-only main   qt6   # exit 1
git -C <main> merge-tree --write-tree --name-only v0.3.1 qt6   # exit 1
git -C <main> merge-tree --write-tree --name-only v0.2.0 qt6   # exit 1
```

| 试验 | 合并树 OID | 未合并路径 | 内容冲突块 | 删改冲突 |
|---|---|---|---|---|
| `main × qt6` | `24d0664` | 14 | **37** | 6 |
| `v0.3.1 × qt6` | `bf7b9c2` | 14 | 36 | 6 |
| `v0.2.0 × qt6` | `089a3dd` | 12 | 18 | 4 |

> `b219bd5` 相对 `v0.3.1` 只多出 `build.yml` 增加 7 行结构与 `build-macos.sh` 的 debug 化，所以 36→37 只差 `build.yml` 的一块。v0.2.0 试验里 `fonts.rs` 不冲突（v0.2.0 未改该文件，qt6 的删除直接生效）。

### 4.2 冲突分类（main × qt6：14 路径 / 43 单元）

**A. 内容冲突（8 文件 / 37 块）**

| 文件 | 块数 | 性质 | 建议解析 |
|---|---|---|---|
| `src/lib.rs` | 9 | 6 块=字段/构造参数双命名；3 块=两套 sysfs 实现 | 以 main 为底 + 保留 qt6 的 `display_name`；统一命名（§5.5） |
| `.github/workflows/build.yml` | 8 | 触发器/输入/矩阵字段/debug env/Windows libwdi 步骤/macOS 步骤/Linux 归档 | 整体重写 |
| `README.md` | 6 | 纯文档：GUI 技术栈、驱动章节、发布策略 | qt6 骨架 + 移植 main 的 Windows 驱动/CI 章节 |
| `README.zh-CN.md` | 5 | 同上 | 同上 |
| `.github/scripts/build-macos.sh` | 4 | `--features gui --bins`(main/egui) vs Qt GUI 段(qt6) | 保留 qt6 Qt 段，吸收 main 的 `RELEASE_FLAG/PROFILE_DIR` |
| `Cargo.toml` | 2 | 版本号；`[features] gui/static-libwdi` vs 无 features | 手工重写（§4.5） |
| `Cargo.lock` | 2 | `auroku 0.2.0-qt6` vs `0.3.1`；`auroku-ffi` vs `auroku-driver-sys`+依赖树 | **重新生成**，不手工合 |
| `.github/scripts/build-linux.sh` | 1 | 整段 GUI 构建：egui(cargo) vs Qt(cmake) | 保留 qt6 的 Qt 段 |

**B. 删改冲突（6 文件）**

| 文件 | 类型 | 说明 | 建议解析 |
|---|---|---|---|
| `src/bin/auroku-gui/app.rs` | qt6 删 / main 改 | main 侧 `+359`（驱动 UI、诊断、启动恢复） | **保留删除**（功能迁 C++） |
| `src/bin/auroku-gui/fonts.rs` | 同上 | main 只把 `eprintln`→`diag::log` | 保留删除 |
| `src/bin/auroku-gui/i18n.rs` | 同上 | main `+80`（16 条驱动文案） | 保留删除，**文案迁到 `gui/src/ui/I18n.cpp`** |
| `src/bin/auroku-gui/main.rs` | 同上 | `windows_subsystem` + `mod diag` | 保留删除（Qt 已 WIN32） |
| `src/bin/auroku-gui/worker.rs` | 同上 | `InstallDriver/RestoreDriver` 请求与 `Event::Driver` | 保留删除，**语义迁到 `RustWorker`** |
| `build-windows.ps1` | qt6 改 / main 删（移到 `.github/scripts/`） | 重命名**未被识别**（内容差异过大） | 取 qt6 内容，落到 `.github/scripts/build-windows.ps1` 覆盖 main 的 egui 版 |

**C. 自动合并但必须复核（`A`/`R` 状态）**

`crates/auroku-driver-sys/{Cargo.toml,build.rs,src/lib.rs}`、`src/driver.rs`(719)、`docs/windows-driver.md`(195)、`AUDIT-REPORT.md`(83)、`src/bin/auroku-gui/{diag.rs,permissions.rs}`（随后删除）、`.github/scripts/windows-cross-build-linux.ps1`（rename + qt6 文案已合）、`.github/scripts/build-windows.ps1`（main 版，需覆盖）、`.github/scripts/build-{linux,macos}.sh`（rename 后冲突）、`.vscode/`、`.gitignore`。

### 4.3 `%TEMP%` 实测（`git merge --no-commit`）

```powershell
$P = Join-Path $env:TEMP 'auroku-merge-probe'
git clone --shared --no-checkout C:/Users/Administrator/Documents/proj/aurokku-rs $P
git -C $P checkout -q -b probe origin/qt6
git -C $P branch -f mainline origin/main
git -C $P merge --no-commit --no-ff mainline      # → 冲突，exit 1
git -C $P status --short
```

输出摘要（与 merge-tree 完全一致）：

```
UU .github/scripts/build-linux.sh        UU .github/scripts/build-macos.sh
UU .github/workflows/build.yml           UU Cargo.lock
UU Cargo.toml                            UU README.md
UU README.zh-CN.md                       UU src/lib.rs
UD build-windows.ps1                     DU src/bin/auroku-gui/{app,fonts,i18n,main,worker}.rs
A  .github/scripts/build-windows.ps1     R  windows-cross-build-linux.ps1 -> .github/scripts/windows-cross-build-linux.ps1
A  AUDIT-REPORT.md                       A  crates/auroku-driver-sys/{Cargo.toml,build.rs,src/lib.rs}
A  docs/windows-driver.md                A  src/driver.rs
A  src/bin/auroku-gui/{diag.rs,permissions.rs}
D  build-linux.sh                        D  build-macos.sh
```

工作区冲突块统计（`^<<<<<<<` 计数 / 总行数）：

| 文件 | 行数 | 块 | 标记行 |
|---|---|---|---|
| `src/lib.rs` | 3277 | 9 | 27 |
| `.github/workflows/build.yml` | 325 | 8 | 24 |
| `README.md` | 430 | 6 | 18 |
| `README.zh-CN.md` | 383 | 5 | 15 |
| `.github/scripts/build-macos.sh` | 224 | 4 | 12 |
| `Cargo.toml` | 53 | 2 | 6 |
| `Cargo.lock` | 481 | 2 | 6 |
| `.github/scripts/build-linux.sh` | 238 | 1 | 3 |
| **合计** | — | **37** | 111 |

> 实验后已 `Remove-Item -Recurse -Force $P` 删除；两个源工作树未被写入（分析结束时 `git -C <main> status --short` 只有本任务的 `?? docs/plan/`）。

### 4.4 `build.yml` 的 8 处冲突（逐块，ours=qt6 / theirs=main）

| # | 行 | qt6（ours） | main（theirs） |
|---|---|---|---|
| 1 | 7 | `on.push.branches=[qt6]`、`tags=["v*"]` | `branches=["**"]`、`tags-ignore=["**"]` |
| 2 | 26 | `inputs.version: required: true` | 新增 `inputs.mode: choice[release,debug]`；`version: required:false` |
| 3 | 84 | `timeout-minutes: 75` | `env.AUROKU_BUILD_DEBUG`（push 或 mode=debug） |
| 4 | 98 | matrix `qt_arch: win64_msvc2022_64` + `asset` | matrix `glob: dist/*.zip` |
| 5 | 108 | Linux/macOS 的 `qt_arch`/`asset` | 对应 `glob` |
| 6 | 154 | 单步 `build-windows.ps1 -Target ...` | `microsoft/setup-msbuild@v2` + `Build static libwdi (WinUSB payload only)` 步骤 |
| 7 | 234 | `bash build-macos.sh` + smoke test + 固定 zip 名 | `bash .github/scripts/build-macos.sh` + `VERSION/SUFFIX` 版本化 zip（**无 smoke test**） |
| 8 | 264 | 静态性校验 + Qt smoke test + 固定 tar 名 | `VERSION/SUFFIX` 版本化 tar |

**自动合并但语义错位的一处**：main 的 `DEBUG_BUILD/PROFILE_DIR/RELEASE_FLAG` 段已合入合并后的 `.github/scripts/build-linux.sh:68-82`，但 qt6 的 GUI 段仍写死 `-DCMAKE_BUILD_TYPE=Release`（合并后同文件 176 行）。汇入后必须让 CMake 跟随 `PROFILE_DIR`，否则 `AUROKU_BUILD_DEBUG=1` 只影响 CLI、GUI 仍是 release。

### 4.5 静默陷阱（无冲突标记，但必须人工处理）

1. **`Cargo.toml` 双 `[workspace]` 表**：合并结果第 1 行是 qt6 的 `[workspace] members=[".","crates/auroku-ffi"]`，第 48 行是 main 的 `[workspace] members=["crates/auroku-driver-sys"]`，**没有标记**。实测（cargo 1.99.1，最小 fixture 复现）：

   ```
   error: duplicate key
     --> Cargo.toml:12:2
      |
   12 | [workspace]
      |  ^^^^^^^^^
   exit=101
   ```

   → 必须手工合成 `members = [".", "crates/auroku-ffi", "crates/auroku-driver-sys"]`。
2. **main 的 `[[bin]] auroku-gui`（`required-features=["gui"]`）与 `eframe` 依赖被静默丢弃**。方向恰好正确（我们要删 egui），但这是「合并副作用」而不是「显式决策」，评审时必须确认。
3. **`src/lib.rs` 的 `run_task` 变成半 qt6 半 main**：合并结果同时有 `config.device_location`（qt6 构造参数）和 `device.set_usb_location(config.usb_location)`（main 新增行）→ **编译错误**。同时 `AuRokkuDevice::new` 的签名保留了 qt6 的额外参数（该行本身未产生冲突标记），而其内部 body 的赋值仍是冲突。这不是 9 个冲突块能覆盖的，必须按 §5.5 统一命名。
4. **`build-windows.ps1` 双份**：根目录 qt6 版 + `.github/scripts/` main（egui）版。

---

## 5. 汇入策略

> 三种策略都从 qt6 HEAD 出发；下列命令序列供实施者执行（本分析**未**执行任何 checkout/merge/commit）。

### 5.1 策略 A：合并分支（推荐）

```powershell
cd C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6
git fetch origin
git switch -c qt6-integrate-0.3.1            # 从 51ff33b 拉出
git merge --no-ff --no-commit b219bd5        # main HEAD（含 v0.3.1）

# 1) egui 侧：一律保留删除（5 个 DU + 2 个自动 A 的 diag/permissions）
git rm -rf src/bin/auroku-gui

# 2) 构建脚本：以 .github/scripts/ 为规范位置
git checkout --ours -- build-windows.ps1                      # 取 qt6 内容
git mv -f build-windows.ps1 .github/scripts/build-windows.ps1  # 覆盖 main 的 egui 版
#    若 git mv 因未合并索引报错：Move-Item -Force 后再 git add + git rm --cached build-windows.ps1
#    .github/scripts/build-linux.sh / build-macos.sh：保留 qt6 的 Qt GUI 段，吸收 main 的 DEBUG/PROFILE_DIR

# 3) Cargo.toml：手工重写（勿逐块挑）
#    - [workspace] members = [".", "crates/auroku-ffi", "crates/auroku-driver-sys"]，只留一个 [workspace]
#    - 删 [features] gui 与 eframe；保留 static-libwdi
#    - 删 [[bin]] auroku-gui
#    - version 统一（建议 0.3.1-qt6，见 §9）

# 4) Cargo.lock：不要手合
cargo generate-lockfile                       # 或 cargo build --workspace 后自动修

# 5) src/lib.rs：以 main 为底 + 叠加 qt6 的 6 处窄改，统一命名为 device_location（§5.5）
# 6) build.yml / README：按新政策重写（不是挑块）

git add -A
git commit -m "merge: sync main 0.3.1 backend into Qt6 front-end"
cargo fmt --all; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
```

**冲突处理要点**

- `src/lib.rs`：只保留**一套** bus/address 机制。建议保留 qt6 的 `device_location`（因为 `crates/auroku-ffi` 已按此命名，而 main 侧唯一使用者 `app.rs` 将被删除）：取 main 的 diff 主体，删掉 main 新增的 `usb_location` 字段 + `set_usb_location()`(587) + `run_task` 那一行(2190)；保留 qt6 的构造参数与 `open_device` 过滤。sysfs 用 **main 的 `fill_strings_from_sysfs`**，删 qt6 的 `read_sysfs_usb_descriptors` 及其 2 个测试（或反之，二选一，**不要两套并存**——两套都在 `scan_devices` 里会被调用两次）。
- `build.yml`：不要逐块合并。以 qt6 三平台矩阵为骨架，插入 main 的 `mode: release|debug` 输入、`AUROKU_BUILD_DEBUG`、libwdi 构建步骤、版本化归档；保留 qt6 的 install-qt-action/Qt 6.8.3、三平台 smoke test、`readelf` 检查。
- `README*`：以 qt6 为准，移植 main 的「Windows 驱动」章节（链接 `docs/windows-driver.md`）与「debug 构建 / 版本化归档」章节。
- `.github/scripts/build-{linux,macos}.sh`：保留 qt6 的 GUI 段（cmake/Qt），吸收 main 的 `PROFILE_DIR/RELEASE_FLAG/AUROKU_BUILD_DEBUG`，并让 CMake 的 `CMAKE_BUILD_TYPE` 跟随。

**优点**：一次解决全部差异；保留完整历史（可 `git log --graph` 追溯）；冲突集中一处可评审；未来 main 前进只需再 merge。
**缺点**：单次 43 个冲突单元，评审体积大；容易漏掉静默陷阱（§4.5）；`src/lib.rs` 与 `Cargo.toml` 必须手工重写。
**建议**：拆成两个提交——① 纯后端同步（Rust/Cargo/CI/脚本，GUI 暂不接新功能）；② FFI + Qt UI 接入驱动能力（§6）。这样 ① 可用 `cargo test` + 三平台 CLI 构建独立验证。

### 5.2 策略 B：cherry-pick 核心提交

```powershell
git switch -c qt6-cherrypick-0.3.1 51ff33b
git cherry-pick <按下列顺序、逐条验证>
```

逐提交评估（23 个，`777c2d2..b219bd5`）：

| 提交 | 内容 | 能干净 pick？ | 说明 |
|---|---|---|---|
| `3a81f4b` | ACL 生命周期 | 否 | 6 个 egui 文件（qt6 已删）+ lib.rs `usb_location` 区 |
| `83ef874` | 审计/clippy | 否 | 改 `permissions.rs`（qt6 无此文件） |
| `298f8d8` | sysfs 设备名 | 否 | app.rs + lib.rs（qt6 已有另一套 sysfs） |
| `b9e51cc` | 版本 0.2.0 | 否 | Cargo.toml/lock/README |
| `108a129` | 静态 libwdi + 脚本迁移 | 否（阻塞整条链） | 19 文件，含 driver.rs 新增（可）、scripts rename、CI、app.rs |
| `b66d3d6` | driver.rs cfg 2 行 | 是（前置：driver.rs 已存在） | |
| `ca8b2d7` | build.yml x64 libwdi | 否 | main 的 libwdi 步骤在 qt6 不存在 |
| `30994bc` | build.yml WDK 路径 | 否 | 同上 |
| `bba951e` | build.yml WinUSB payload | 否 | 同上 |
| `b4afdb2` | build.yml LIBUSB 宏 | 否 | 同上 |
| `ac7494a` | build.yml libwdi_static | 否 | 同上 |
| `625f489` | driver-sys build.rs 链接 shell32 | 是（前置：crate 已存在） | |
| `7f5ca5e` | driver-sys 先生成 INF | 是 | |
| `66ad7d3` | driver-sys 传 HWID | 是 | |
| `a38115b` | driver.rs restore 删驱动包 | 是 | |
| `05bd642` | driver.rs UTF-16 INF 匹配 | 是 | |
| `ccd4447` | lib.rs Windows 名字解析 | 否 | 与 qt6 sysfs 段相邻，1–3 处冲突 |
| `b5e78b7` | lib.rs cfg_attr 1 行 | 否（大概率） | 同上区域 |
| `78687fa` | lib.rs 非 ASCII 解码 | 否 | 同上区域 |
| `c9a75d4` | driver.log + 隐藏提权窗口 | 部分 | driver.rs 干净；lib.rs/app.rs/i18n/worker 冲突 |
| `ba75b4c` | Windows 子系统 + gui.log | 否 | 全在 egui 文件 |
| `77e1064` | CI 版本化 + 0.3.1 | 否 | build.yml/Cargo |
| `b219bd5` | debug 构建策略 | 否 | build.yml/README |

**结论**：只有 9 个提交可能干净，其中 6 个必须先让 `108a129` 的 driver 栈落地，而 `108a129` 本身不可 pick。实际路径退化为「cherry-pick 9 个后端提交 + 手工移植 `108a129` 的 driver 部分」= 策略 C 的变体。
**优点**：历史粒度清晰，每个提交可单独验证；不产生巨型合并提交。
**缺点**：14/23 必然冲突；`108a129` 断链；最终仍要手工 merge `build.yml`。

### 5.3 策略 C：diff 补丁（`git diff v0.2.0..main -- <paths>`）

```powershell
git switch -c qt6-patch-0.3.1 51ff33b
# 只取 qt6 完全没有的路径，避免与 qt6 的重叠改动相撞
git diff v0.2.0..main -- src/driver.rs crates/auroku-driver-sys docs/windows-driver.md > $env:TEMP/driver.patch
git apply --3way --check $env:TEMP/driver.patch
git apply --3way $env:TEMP/driver.patch
git diff v0.2.0..main -- src/lib.rs > $env:TEMP/lib.patch
git apply --3way --reject $env:TEMP/lib.patch   # lib.rs 必然有 reject，逐个手工
```

**适用面**：driver 栈（`src/driver.rs`、`crates/auroku-driver-sys`、`docs/windows-driver.md`）非常干净——这些路径 qt6 完全没有，等价于新增文件。`src/lib.rs` 的补丁不适用（qt6 已在 sysfs/字段区域改动，3way 仍产 reject）。
**优点**：可控、可分批、不必解决 43 个冲突；不引入 main 的历史。
**缺点**：丢失与 main 的祖先关系（之后每次同步都要重来）；`build.yml`/`README`/CI 仍要手工并；`Cargo.toml` 仍要手工。
**建议**：作为**辅助手段**（策略 A 第 ② 步之后补 driver 后端的兜底），不作为主路径。

### 5.4 对比与推荐

| 维度 | A 合并 | B cherry-pick | C diff 补丁 |
|---|---|---|---|
| 一次解决度 | 高 | 低 | 中 |
| 冲突总量 | 43 单元（集中一次） | 14 提交必冲突 + 9 可干净 | 仅 lib.rs 需手工 |
| 历史可追溯 | 是（完整） | 部分 | 否（脱离 main） |
| 后续同步成本 | 低（再 merge） | 低 | 高（每次重做） |
| 出错风险 | 中（静默陷阱 §4.5） | 高（链断） | 中 |
| 工时估计 | 0.5–1 天（含编译修复） | 1–2 天（逐条） | 0.5 天（仅后端） |

**推荐：策略 A，分支名 `qt6-integrate-0.3.1`，第一个提交只同步后端、第二个提交接 FFI/UI。** 若实施中发现 `src/lib.rs` 合并过于纠缠，可 `git merge --abort` 回到 qt6，改用「策略 A 的脚本/CI 部分 + 策略 C 的 driver 栈」混合方案。

### 5.5 推荐执行序列（可直接照抄）

```powershell
# 0) 预备
cd C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/qt6
git status --short                 # 必须干净
git fetch origin; git log --oneline -1 b219bd5

# 1) 拉分支 + 合并（不提交）
git switch -c qt6-integrate-0.3.1
git merge --no-ff --no-commit b219bd5

# 2) 删 egui（5 个 DU + 2 个自动 A）
git rm -rf src/bin/auroku-gui

# 3) 脚本落位（见 §5.1）
git checkout --ours -- build-windows.ps1
git mv -f build-windows.ps1 .github/scripts/build-windows.ps1

# 4) 手工改 4 个文件（不要用 checkout --ours/--theirs 整体取）
#    Cargo.toml   → [workspace] 合并 members、删 [features] gui/eframe、删 [[bin]] auroku-gui
#    src/lib.rs   → 以 main 为底；统一 device_location；删 usb_location/set_usb_location；sysfs 二选一
#    build.yml    → 重写（qt6 矩阵 + main debug/release + libwdi + smoke test + 版本化归档）
#    README*      → qt6 骨架 + main 的驱动/CI 章节

# 5) 锁文件
cargo generate-lockfile

# 6) 编译与修复（git 看不到的错，编译器能看到）
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build -p auroku-ffi --release

# 7) GUI 冒烟
cmake -S gui -B build/gui -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=<Qt>
cmake --build build/gui
# Linux: QT_QPA_PLATFORM=offscreen ./build/gui/auroku-gui --smoke-test

# 8) 提交（后端同步）
git add -A
git commit -m "merge: bring main 0.3.1 backend (driver/libwdi, device names, debug CI) into qt6"

# 9) 第二个提交：FFI + Qt UI（§6）
```

**`src/lib.rs` 命名统一清单（缺一即编译失败）**

| 位置 | 处理 |
|---|---|
| `DeviceConfig` 字段 | 保留 `device_location`（qt6） |
| `DeviceConfig::new` | `device_location: None` |
| `AuRokkuDevice` 字段 | 保留 `device_location` |
| `AuRokkuDevice::new` | 保留第 3 参数 `device_location: Option<(u8,u8)>` |
| `AuRokkuDevice::set_usb_location` | **删除**（main 新增，qt6 用构造参数） |
| `run_task` | 删掉 `device.set_usb_location(config.usb_location);` 这一行 |
| `cli_main` 字面量 | `device_location: None` |
| `crates/auroku-ffi/src/lib.rs` | **不需要改**（已写 `config.device_location`） |

---

## 6. 汇入后 FFI 与 C++ 改动点

### 6.1 `crates/auroku-ffi`：新增 4 个 extern C 函数（ABI 1 → 2）

```c
/* 与既有约定一致：JSON 信封 + auroku_string_free 释放 + 可选日志回调 */
char *auroku_driver_status(void);
/*   data = { "supported": bool, "pending": DriverRecord|null,
              "log_path": "<state_dir>/driver.log", "state_dir": "<state_dir>" } */
char *auroku_driver_install(int vid, int pid, int restore_on_exit,
                            AurokuLogCallback cb, void *user_data);
/*   data = DriverRecord { vid, pid, installed_at, restore_on_exit, restored }
     失败 kind = "DriverError"（透传 driver::install 的 String 错误） */
char *auroku_driver_restore(int vid, int pid, AurokuLogCallback cb, void *user_data);
/*   data = null；失败同样 "DriverError" */
char *auroku_state_dir(void);
/*   data = 字符串路径（供 C++ 写 gui.log，等价 main 的 driver::state_dir()） */
```

实现要点（与 main 的对应关系）：

- `driver::install(vid, pid, restore_on_exit, log: Option<DriverLog>, append_log=false)`（`main:src/driver.rs:293`）→ 用 `LogBridge` 把驱动日志实时转成回调（复用 FFI 里现成的 `LogBridge`）。
- `driver::restore(vid, pid, log, append_log=false)`（`337`）。
- `driver::pending_restore()`（`136`）、`driver::log_path()`（`227`）、`driver::state_dir()`（`75`）→ `auroku_driver_status`。
- **不要**把驱动操作塞进 `auroku_run_task` 的 `task.kind`：`run_task` 会构造 `AuRokkuDevice` 并在 `DeviceConfig` 语义下开设备会话，而驱动安装恰恰要在「设备被 usbser.sys 占用、libusb 打不开」时执行，且可能阻塞数十秒等 UAC。独立函数语义更准。
- **feature 透传**：`auroku-driver-sys` 只在 `static-libwdi` 打开时编入。给 FFI crate 加 `[features] static-libwdi = ["auroku/static-libwdi"]`，并在 CMake 的 cargo 调用里按平台加 `--features auroku-ffi/static-libwdi`（Windows CI）。否则 GUI 走 `wdi-simple.exe` 回退路径——而发布包里**没有** `wdi-simple.exe`（main 从 `108a129` 起就改用静态 libwdi）。
- **C++ 链接面**：libwdi 的系统导入库**不会**随 Rust `staticlib` 自动传播。`gui/CMakeLists.txt` 的 Windows 分支要补 `setupapi ole32 ntdll advapi32 user32 gdi32 comctl32 crypt32 wintrust version shlwapi cfgmgr32 msi shell32 oleaut32 uuid comdlg32 winspool userenv`（即 `crates/auroku-driver-sys/build.rs` 的清单），并把 `LIBWDI_LIB_DIR/libwdi.lib` 加进 `target_link_libraries`。

### 6.2 JSON 协议扩展

| 变化 | 说明 | C++ 改动 |
|---|---|---|
| 错误 kind 新增 `"DriverError"` | 来自 `AuRokkuError::Driver`；`open_failure` 会带「装 WinUSB 驱动 / 或用 Zadig」的可操作文案（`main:src/lib.rs:85-105`） | `RustWorker`/`AppState` 已保存 `kind`；`MainWindow::startTask` 失败处理在 `kind=="DriverError"` 时置位 `driverPrompt` 并跳转/展开 Overview 驱动卡片 |
| `scan_devices` 设备对象 | **结构不变**（vid/pid/bus/address/manufacturer/product/serial/id/display）；合并后可拿到 Windows 注册表名与非 ASCII 文案 | 无需改（`DeviceInfo::fromJson` 已有） |
| 请求里的 `bus`/`address` | 保持（qt6 已用）；对应 lib.rs 的 `device_location` 命名统一 | 无需改 |
| 新增 driver 三函数的返回值 | `DriverRecord` JSON（字段同 `driver::DriverRecord`） | `AppState` 新增 `driver` 子状态 |

### 6.3 页面 / C++ 改动点（对照 main 的 egui 新增功能）

| main 的 egui 改动 | 位置 | Qt 侧落点 |
|---|---|---|
| 驱动卡片（安装/恢复、恢复开关、「当前由 usbser.sys 占用」横幅、driver.log 路径） | `app.rs:453,460,647,663` + `i18n.rs` 16 条文案 | **OverviewPage** 新增 `buildDriverCard()`：安装/恢复按钮、`ToggleSwitch` 退出时恢复、当前 service 标签、可复制 `driver.log` 路径；仅 Windows 显示；文案直接复用 main 已翻译的 16 条 |
| 安装成功自动重试上次操作 | `app.rs:460` 附近（「WinUSB 驱动已安装，正在重试…」） | `MainWindow` 记住 `m_lastTask`，`driverInstalled` 后重发 |
| 启动时 `pending_restore` 告警 + 退出时自动恢复 | `app.rs:291,2790` | `MainWindow` 构造末尾查 `auroku_driver_status`，有 pending 则 toast/横幅；`closeEvent` 里若 `restoreOnExit && pending` 调 `auroku_driver_restore`，失败阻止关闭（与现有 ACL revoke 语义一致） |
| `gui.log`（release 无控制台） | `diag.rs` | `main.cpp` 安装 `qInstallMessageHandler`，把 `qDebug/qWarning/qCritical` + 自定义 `diagLog()` 写入 `auroku_state_dir()/gui.log`（启动截断）；把现有 `fprintf(stderr)`（`UsbAccess.cpp` 析构）改走它 |
| `DriverError` 提示 | `main:src/lib.rs:85` | ConsolePage/状态栏展示 `kind: message`（现有 `Envelope::describe()` 已够） |
| 非 ASCII 设备名 | `main:src/lib.rs:1943` | 无需改 C++（JSON 已是 UTF-8） |

### 6.4 `RustWorker`/`RustBridge` 线程模型够不够？

**够用，但要改 3 处**：

1. **busy 语义**：`RustBridge::runAsync` 在 `m_busy` 时静默 `return`（`RustBridge.cpp:55-58`）。驱动安装可能占用 worker 30s–2min（等 UAC），期间所有协议操作会被静默丢弃。改为发 `taskRejected` 信号，或让上层 `startTask` 先判 busy 并 toast（`MainWindow::startTask:464` 已部分做了，`scanAsync` 未做）。
2. **新增槽/信号**：`RustWorker` 加 `driverStatus()`、`driverInstall(vid,pid,restoreOnExit)`、`driverRestore(vid,pid)`；信号复用 `logLine` + 新增 `driverFinished(op, ok, data, kind, message)`；`RustBridge` 加对应 `*Async` 包装。
3. **取消/超时**：Qt 侧目前没有取消机制；驱动安装等 UAC 时用户可能点关闭。建议 `closeEvent` 在 busy 且是 driver 操作时提示「正在安装驱动，请等待」，或给 `auroku_driver_install` 加 `timeout_ms`（Rust 侧 `wait_until` 已是 30s 上限，UAC 等待由 Windows 决定）。

> 单 worker 线程 + 串行执行对七个页面 + 驱动操作**足够**：USB 与驱动操作本就互斥，不需要线程池。**不要**为驱动操作新开线程（会与 ACL/设备生命周期交叉）。

---

## 7. qt6 侧必须保留 / 合并时易误删

### 7.1 必须保留（合并时采用 ours/qt6 或完全不动）

| 类别 | 文件 | 理由 |
|---|---|---|
| Qt GUI 全部 | `gui/**`（45 文件） | main 完全没有；任何 `checkout --theirs` 误操作都可能删目录 |
| C ABI 桥 | `crates/auroku-ffi/**` + `Cargo.toml` 的 members | 漏掉 `crates/auroku-ffi` → CMake 的 `-p auroku-ffi` 直接失败 |
| Qt 构建脚本 | `.github/scripts/build-{windows.ps1,linux.sh,macos.sh}` 的 Qt 段、`windows-cross-build-linux.ps1` 的 Qt 文案 | 被 main 的 egui 版覆盖即回退 |
| CI qt6 专属步骤 | `jurplel/install-qt-action@v4`(Qt 6.8.3) + 三平台 `qt_arch`；Linux 的 Qt/X11/xcb 依赖 apt 列表；三平台 `--smoke-test`；`readelf -d ... \| grep libauroku_ffi` 断言；构建机路径检查；`macos-universal` 的 `ditto` 打包 | 这些是 qt6 相对 main 的净增量 |
| 静态 Qt/Wayland | `CMakeLists.txt` 的 `qt_import_plugins` 块（126-134）+ `main.cpp` 的 `AUROKU_HAVE_WAYLAND_PLUGIN` 分支 | `51ff33b` 的成果 |
| 无 libudev | CMake 的 `PKG_CONFIG=/bin/false` + `build-linux.sh` 的 `env -u PKG_CONFIG` | 影响静态链接 |
| FFI 静态链接 | CMake `add_library(... STATIC IMPORTED ...)` + 脚本里的 `! readelf ... libauroku_ffi` 断言 | `519b1a6` 的成果 |
| 版本后缀 | `CMakeLists.txt` 的 `AUROKU_VERSION_SUFFIX "-qt6"` | 与 `v0.2.0-qt6` 约定一致 |
| 配置 | `.vscode/settings.json`、`.gitignore` 的 `/build` 与 `/.vscode` | qt6 独有；注意 `settings.json` 里是**绝对路径**（`/home/zsyizh/...`），建议改相对或删除 |
| 文档 | `SECURITY_AUDIT.md`、`gui/README.md`、README 的 Qt 章节 | main 对应物是 `AUDIT-REPORT.md`（内容不同，不要互相覆盖） |

### 7.2 易误删 / 易回退清单

1. **批量 `git checkout --theirs -- .`**：不会删 `gui/`（main 没这路径），但会把 `.github/scripts/build-*.{sh,ps1}` 全换成 egui 版、把 `build.yml` 换成 main 版 → Qt 步骤全丢。
2. **`src/bin/auroku-gui/` 只删一半**：main 自动加入 `diag.rs`/`permissions.rs`；若只 `git rm` 5 个 DU 文件，目录里会残留这两个 + 「Version main left in tree」写回工作区的旧文件。建议 `git rm -rf src/bin/auroku-gui` 并物理删除。
3. **`Cargo.toml` 忘记并 members**：只留 main 的 `["crates/auroku-driver-sys"]` → `auroku-ffi` 掉出 workspace → CMake 构建失败。
4. **`[[bin]] auroku-gui` 残留**：bin 指向已删路径，`cargo build --workspace` 报「can't find src/bin/auroku-gui/main.rs」。
5. **`[features] gui = ["dep:eframe"]` 残留**：需要重新引入 eframe 依赖，`cargo test`/clippy 报错。
6. **`.gitignore` 合并**：`/build` 被忽略是对的；误加 `/crates`、`/gui` 会直接盖住源码（评审时看 `.gitignore` diff）。
7. **`build-windows.ps1` 双份**：见 §4.5-4；合并后两份只能留一份。
8. **`Cargo.lock` 手工合并**：会丢 `auroku-ffi` 或 libwdi 相关依赖（`cargo build --locked` 立刻失败）。永远重新生成。

---

## 8. 风险与验证清单

### 8.1 高风险点

| 风险 | 触发方式 | 影响 | 缓解 |
|---|---|---|---|
| 静默丢 main 的新增行 | 3-way 把 `[[bin]] auroku-gui`/`eframe` 吃掉（已实测） | 评审误以为已合、实际缺功能 | 合并后用 `git diff b219bd5 <merge-result> -- <path>` 反向核对关键文件；`Cargo.toml` 逐行 diff 而不只看冲突标记 |
| `src/lib.rs` 半死不活 | 字段命名各留一半（已实测） | 编译失败，但 `git status` 显示「已解决」 | 用 `cargo check` 作唯一仲裁；按 §5.5 清单逐项确认 |
| `Cargo.toml` 非法 | 双 `[workspace]`（已实测 exit 101） | 整个 workspace 不可用 | 合并后第一条命令跑 `cargo metadata --no-deps` |
| 驱动后端没编进来 | `static-libwdi` feature 未透传到 FFI/CMake | Windows 包内无 libwdi，安装时回退到不存在的 `wdi-simple.exe` | CI 断言 GUI 二进制不依赖 `wdi-simple`；或真机跑一次安装 |
| ACL 语义回退 | 误把 `UsbAccess` 换回 main 的 `permissions.rs`（后者随 egui 删除） | 多设备/root 校验能力丢失 | 保留 `UsbAccess`，补 `refuse_root_execution` 到 `main.cpp` |
| Linux GUI 变动态/找不到插件 | `build-linux.sh` 的 `env -u` 清理被 main 版覆盖 | smoke test 挂 | 保留 `env -u RUSTFLAGS -u C_INCLUDE_PATH -u PKG_CONFIG` 三处 |
| 调试构建只影响 CLI | `AUROKU_BUILD_DEBUG` 未传给 CMake | debug 构件里 GUI 仍是 release | `-DCMAKE_BUILD_TYPE` 跟随 `PROFILE_DIR` |

### 8.2 合并后验证清单（按顺序）

**静态 / 编译**

1. `git grep -n -E "eframe|egui|src/bin/auroku-gui"` → 源码 0 命中（只允许出现在 docs 的历史说明里）。
2. `git ls-files | grep -E "^build-(windows|linux|macos)|^gui/|^crates/auroku-ffi"` → qt6 资产齐全；`build-*.{sh,ps1}` 只有一份。
3. `cargo metadata --no-deps --offline` → 无 duplicate key，members 含 3 个路径。
4. `cargo fmt --all -- --check`。
5. `cargo clippy --workspace --all-targets -- -D warnings`（注意 FFI crate 的 `#![allow(unsafe_code)]`；`crates/auroku-driver-sys` 的 Windows-only 代码在 Linux 上要 `#[cfg]` 干净——main 有专门提交 `b66d3d6` 修这个）。
6. `cargo test --workspace --all-targets`（含 qt6 的 2 个新 lib 测试 + FFI 5 个 + driver.rs 的 `#[cfg(test)]`）。
7. `cargo build -p auroku-ffi --release` 产出 `libauroku_ffi.a` / `auroku_ffi.lib`。

**三平台构建 + smoke**

8. Linux：`./.github/scripts/build-linux.sh` → CLI `file` 为 static、无 `PT_INTERP`/`DT_NEEDED`；GUI `readelf -d` 不含 `libauroku_ffi`；`QT_QPA_PLATFORM=offscreen ./dist/auroku-gui-linux-x86_64 --smoke-test` 退出码 0。
9. Windows：`.github/scripts/build-windows.ps1 -Target x86_64-pc-windows-msvc`（本地需 Qt + Ninja；CI 先构 libwdi）；`dumpbin /DEPENDENTS` 对 **CLI 与 GUI** 都只含系统 DLL（静态 CRT 断言，建议移植 main 的 `Assert-StaticRuntime`）；`QT_QPA_PLATFORM=offscreen dist/auroku-gui-windows-x86_64/auroku-gui.exe --smoke-test`。
10. macOS：`bash .github/scripts/build-macos.sh` → CLI `lipo -info` 为 x86_64+arm64；`otool -L` 的 GUI 不含 `libauroku_ffi.dylib`；`QT_QPA_PLATFORM=offscreen dist/auroku-gui.app/Contents/MacOS/auroku-gui --smoke-test`。
11. Qt 静态链接专项：`ldd dist/auroku-gui-linux-x86_64 | grep -i qt` 结果符合预期；`nm -D` 不出现未定义 `auroku_*` 符号。
12. 发布产物命名：`dist/auroku-linux-x86_64-v0.3.1-qt6.tar.gz` 等（版本化 + debug/release 区分）。

**功能验证（需要设备 / 真机）**

13. Windows：设备被 usbser.sys 占用 → GUI 报 `DriverError` → 点「安装 WinUSB 驱动」（一次 UAC）→ `driver.log` 出现 install 段 → 自动重试成功；勾选「退出时恢复」后关闭 → 恢复 usbser 并清空 `driver-state.json`；强杀进程后重启 GUI → 出现 pending_restore 告警。
14. Linux：选中设备 → `getfacl /dev/bus/usb/BBB/DDD` 出现 `user:<uid>:rw-` 且 mask 未被放大；关闭 GUI → ACL 复原；`kill -9` → 残留（`SECURITY_AUDIT.md` 已记录该已知限制）。
15. 设备名：Windows 下 usbser 绑定的设备在扫描列表显示中文/非 ASCII 产品名（`ccd4447`/`78687fa` 验收点）；Linux 下未授权时也能显示 sysfs 名。
16. `gui.log`：release 版启动后 `%LOCALAPPDATA%/auroku/gui.log` 有内容（等价 main 的 `ba75b4c` 行为）。

**CI**

17. `build.yml` 语法自检（`actionlint`，或 GitHub 侧 `workflow_dispatch mode=debug` 试跑：只出 Actions 构件、不出 Release）。
18. `workflow_dispatch mode=release version=v0.3.1-qt6` 试跑：产物齐全、tag 正确、命名含版本。
19. PR 触发：`pull_request` 只跑 `test` job（`build` 有 `if: github.event_name != 'pull_request'`）。

---

## 9. 未决问题（需要 Lead/用户决策）

1. **合并方向**：推荐 `main → qt6`（qt6 为第一父）。是否接受 qt6 长期作为主线、main 变成「后端来源」？
2. **egui 去留**：合并即删除 `src/bin/auroku-gui/*` 与 `gui`/eframe feature。是否要在 main 上保留一段时间（双 GUI）以免回退困难？
3. **版本号**：`0.3.1-qt6`（沿用 qt6 惯例）还是 `0.3.1`（与 main 对齐）？影响 Cargo.toml/Cargo.lock、CMake `AUROKU_VERSION_SUFFIX`、CI 版本化文件名与 tag。
4. **发布名**：`v0.3.1-qt6` 还是 `v0.4.0`（Qt GUI 取代 egui，属用户可见的破坏性变化）？
5. **Qt 运行时策略**：CI 用 install-qt-action 的动态 Qt 6.8.3 + windeployqt/macdeployqt；维护者本地是静态 Qt。发布产物到底「动态 Qt + 插件目录」还是「静态 Qt 单文件」？影响 `qt_import_plugins` 是否在生产构建生效、包体大小与许可证说明。
6. **macOS GUI 是否要 universal**：现在 Qt GUI 只出本机架构；main 的 egui GUI 是 lipo 通用。要不要补两架构 Qt 构建（CI 时间翻倍）？
7. **多设备 ACL**：qt6 `UsbAccess` 单设备。是否需要「同时保持多台设备授权」（需把 `UsbAccess` 改成管理列表，或在 `MainWindow` 放多个实例）？
8. **`AUDIT-REPORT.md`（main）与 `SECURITY_AUDIT.md`（qt6）合并**：两份文档覆盖不同时期/范围，是否本次一并合并？
9. **`.vscode/settings.json` 绝对路径**（`/home/zsyizh/...`）：删除、改相对路径、还是移出版本控制？
10. **`docs/windows-driver.md` 的 Qt 适配**：main 的文档描述 CLI/egui 流程（`--op install-driver`、wdi-simple），Qt GUI 上线后需补「GUI 一键安装」章节。
11. **libwdi LGPL 合规**：main 在包内放 `libwdi/COPYING-LGPL`；Qt 包是 zip 目录结构，许可证文件放哪、README 如何声明，需要确认。

---

## 附录 A：本文用到的只读命令（可复现）

```bash
# 基线
git -C <main> log --oneline -40 --decorate
git -C <qt6>  log --oneline -30 --decorate
git -C <main> tag -l --format='%(refname:short) %(objecttype) %(*objectname) %(objectname)'

# diffstat / name-status
git -C <qt6>  diff --stat        777c2d2..qt6
git -C <main> diff --stat        777c2d2..main
git -C <qt6>  diff --name-status 777c2d2..qt6

# 函数级
git -C <qt6>  grep -n '^\s*(pub )?(async )?fn |^pub struct |^pub enum |^impl ' qt6 -- src/lib.rs
git -C <main> grep -n '^\s*(pub )?(async )?fn |^pub struct |^pub enum |^impl ' main -- src/lib.rs
git -C <qt6>  diff 777c2d2..qt6 -- src/lib.rs

# 试合并（只读，不落地）
git -C <main> merge-tree --write-tree --name-only main   qt6
git -C <main> merge-tree --write-tree --name-only v0.3.1 qt6
git -C <main> merge-tree --write-tree --name-only v0.2.0 qt6
git -C <main> cat-file -p "<tree>:src/lib.rs" | grep -c '^<<<<<<<'

# 冲突块统计（%TEMP% 克隆，实验后删除）
git clone --shared --no-checkout <main> $env:TEMP/auroku-merge-probe
git -C $env:TEMP/auroku-merge-probe checkout -q -b probe origin/qt6
git -C $env:TEMP/auroku-merge-probe branch -f mainline origin/main
git -C $env:TEMP/auroku-merge-probe merge --no-commit --no-ff mainline
git -C $env:TEMP/auroku-merge-probe status --short
Remove-Item -Recurse -Force $env:TEMP/auroku-merge-probe
```

**校验记录**：两个源工作树在本次分析后未被修改（`git -C <main> status --short` 仅 `?? docs/plan/`；`git -C <qt6> status --short` 为空）；`%TEMP%/auroku-merge-probe`、`%TEMP%/auroku-toml-probe` 已删除。
