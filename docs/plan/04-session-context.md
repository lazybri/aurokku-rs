# 04 · 本次会话上下文（Windows 驱动栈 + CI 改造）

> 生成于 2026-11-04；基线 `main` HEAD **b219bd5**，crate 版本 **0.3.1**，已发布 **v0.3.1**。
> 本文只记录「会话上下文」：做了什么、为什么、验证结果、坑与待办；实现细节以代码为准。
> 相关：`01-main-history-0.2.0-0.3.1.md`（提交史/移植单元）、`02-qt6-gap-analysis.md`（qt6 差距）、`03-libwdi-decoupling.md`（libwdi 解耦）。

---

## 0. 一句话结论

Windows 上「设备被 usbser.sys 整机占用、libusb 打不开」的问题已经闭环：发布版在进程内用**静态链接的 libwdi** 安装 WinUSB、装完自动重试、可一键恢复；CI 改为「push 只出 debug 构件、手动 dispatch 才发 Release」，产物名带版本号。

---

## 1. 会话目标演进

1. 最初问题：Windows 需要 Zadig 注入 WinUSB 才能控制 USB 设备，用户设备插上直接出 COM 口。
2. 定位：设备是 CDC 复合设备，Windows 用 `usbser.sys` 占整机（IF0+IF1），SERI 协议在 IF2（bulk `0x04`/`0x84`），libusb 无法打开。
3. 方案：**静态链接 libwdi**，进程内安装 WinUSB（不再随包发 `wdi-simple.exe`），CI 构建这份 C 库。
4. 随后补齐：UAC 体验、驱动卸载、设备名回退、日志落盘、GUI 子系统、CI 的 debug/release 双模式。

---

## 2. 关键实现（当前代码状态）

### 2.1 驱动层 `src/driver.rs`

- `install(vid, pid, restore_on_exit, log, append_log)`：
  - 非 append 时**截断** `driver.log` 并写头部；
  - `install_backend`：`static-libwdi` 走 `auroku_driver_sys::install_winusb`，否则回退外部 `wdi-simple.exe`；
  - `WDI_ERROR_NEEDS_ADMIN` → `elevate_self(&["--op","install-driver",...,"--driver-log-append"])`；
  - `wait_until(openable=true, 30s)` 轮询 `rusb` 能否打开，成功后写 `driver-state.json`。
- `restore(vid, pid, log, append_log)`：
  - 提权后**先删除所有匹配 VID/PID 的 `oemNN.inf` 驱动包**（`pnputil /delete-driver ... /uninstall /force`），再 `pnputil /remove-device` + `/scan-devices`；
  - `wait_until(openable=false, 30s)` 要求读到**真实存在且非 WINUSB** 的 service。
- 辅助：`log_path()`（`%LOCALAPPDATA%\auroku\driver.log`）、`state_path()`、`pending_restore()`、`driver_packages()`（ASCII + UTF-16LE 双模式匹配 INF）、`device_instance_ids()`、`can_open()`。
- 错误类型：`AuRokkuError::Driver`（kind `DriverError`），GUI 据此显示安装入口。

### 2.2 FFI crate `crates/auroku-driver-sys`

- 手写 libwdi 绑定：`wdi_create_list` / `wdi_destroy_list` / `wdi_prepare_driver` / `wdi_install_driver` / `wdi_strerror` / `wdi_set_log_level`；
- Win32：`is_elevated`（OpenProcessToken）；
- `install_winusb` 流程：建 `%TEMP%\auroku-wdi` → `wdi_prepare_driver(WDI_WINUSB, "usb_device.inf")` → 用 `wdi_create_list` 查 `hardware_id`（失败则拼 `USB\VID_xxxx&PID_yyyy`）→ `wdi_install_driver`；
- `build.rs`：`LIBWDI_LIB_DIR` + `static=libwdi` + setupapi/ole32/ntdll/…/shell32 等 19 个系统库。

### 2.3 GUI（egui）

- 驱动失败时「最近结果」卡片显示原因 + **安装 WinUSB 驱动** / **恢复原驱动** + 「退出时自动恢复」复选框；安装成功自动重试上次操作；
- release 构建是 Windows GUI 子系统（`#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`），不再有控制台窗口；
- 诊断走 `src/bin/auroku-gui/diag.rs`：stderr + `%LOCALAPPDATA%\auroku\gui.log`（启动截断）；
- 控制台模块会打印 `驱动日志：<path>`。

### 2.4 设备名回退 `src/lib.rs`

- 描述符读不到时（设备被占用/无权限）用注册表 `FriendlyName`/`DeviceDesc`/`Mfg` + `reg query`；
- 非 ASCII 名字若被控制台代码页破坏（出现 `U+FFFD`），改用 PowerShell（`[Console]::OutputEncoding=UTF8`）重读；
- 按扫描缓存，装/卸驱动后下一次扫描即更新。

### 2.5 CI `.github/workflows/build.yml`

- **push 任意分支** → debug 构建（dev profile），只上传 Actions 工件，名称 `*-debug`，**不发布 Release**；
- **workflow_dispatch** 提供 `mode`：
  - `release`：优化构建 + 发布 GitHub Release（需填 `version`，可 draft/prerelease）；
  - `debug`：同 push 的 debug 构建；
- Windows job 内联编译 libwdi：下 WDK redist MSI（`msiexec /a` 解包）→ clone libwdi v1.5.1 → 注释 `OPT_M32/OPT_ARM/LIBUSB0_DIR/LIBUSBK_DIR` → 依次编 `detect_64build`/`installer_x64`/`embedder` → 删 `libwdi_static` 的 ProjectReference → 编静态库 → 导出 `LIBWDI_LIB_DIR`；
- 打包：产物带版本号 `auroku-<platform>-v0.3.1[-debug].zip|tar.gz`；Windows 附 `LICENSE-LGPL-3.0.txt`；
- 静态 CRT 校验：`dumpbin /dependents`，禁止 `vcruntime*/msvcp140*/ucrtbase/api-ms-win-crt-*/libgcc_s_*`。

---

## 3. 实测结论（设备 `04C5:1270`，au T008）

| 项 | 结果 |
| --- | --- |
| 拓扑 | IF0 CDC comm(0x82)；IF1 CDC data(`0x03/0x83`)；IF2 厂商口(`0x04/0x84`，SERI) |
| Windows 绑定 | 设备级 class 02 → `usbser.sys` 占整机，无 `&MI_` 子节点 |
| COM 口 | 能创建但 `SerialPort.Open` 报 `0x8007001F`，`usbser` 功能不可用 |
| libusb 打开 | `Operation not supported`（被 usbser 占用） |
| 安装后 | `Service = WINUSB`，`probe --probe-kind seri` 回显 `1c 05` |
| 恢复后 | `Service = usbser`，`USB 串行设备 (COMx)` 回来（COM 号会变） |

日志：`%LOCALAPPDATA%\auroku\driver.log`（每次操作截断）、`gui.log`、`driver-state.json`。

---

## 4. 踩坑清单（现象 → 原因 → 修法）

1. **`libwdi error -2`**：`wdi_install_driver` 的 `path`/`inf` 不能为 NULL（内部要启动同目录的 `installer_x64.exe`）。→ 先 `wdi_prepare_driver` 生成 INF/CAT 再安装。
2. **装完 Service 仍是 usbser**：`device_info->hardware_id` 为空，installer 不调用 `UpdateDriverForPlugAndPlayDevices`，只把包复制进驱动仓库。→ 用 `wdi_create_list` 查真实 hardware ID。
3. **卸载“成功”但仍是 WinUSB**：①只删设备节点，驱动包还在，PnP 重新绑回；②`wait_until` 把“读不到 service”当成功；③INF 是 **UTF-16LE**，ASCII 字节搜不到 `oemNN.inf`。→ 删包 + 只认真实非 WINUSB service + 双字节模式匹配。
4. **驱动包签名失败 `0x800b0109`**：证书/签名时序问题（曾先 prepare 后又被提权子进程重做）。最终方案下 install 在提权进程内一次完成，未再复现。
5. **设备名显示方框**：`reg.exe` 输出是控制台代码页（GBK），按 UTF-8 解码成 `U+FFFD`。→ PowerShell UTF-8 重读。
6. **GUI 多出一个黑窗口**：`auroku-gui.exe` 没有 windows 子系统声明。→ `windows_subsystem` + `gui.log`。
7. **macOS debug 构建失败**：bash 3.2 在 `set -u` 下展开空数组报 `unbound variable`。→ 用标量 `RELEASE_FLAG`。
8. **libwdi 构建报 MSB8013**：`embedder`/`installer_x86` 只有 Win32 配置，直接编 vcxproj 会按 x64 解析。→ 分别用正确平台编译，并删除静态工程的 ProjectReference。
9. **`WDK_DIR` 路径被转义**：`\a`/`\w` 被 MSVC 当转义序列。→ 宏里用正斜杠。
10. **链接缺 `ShellExecuteExW`/`SHCreateDirectoryExW`**：静态库不自动带系统库。→ 显式链接 shell32 等。
11. **设备“离场”时安装报 `-99`**：设备 `Disconnected` 时 libwdi 返回未映射错误。**待改进**：安装前检查设备在场，给出“设备未连接”。

---

## 5. 当前仓库/发布状态（截至 2026-11-04）

- 分支：`main`（HEAD `b219bd5`）、`qt6`（旧 GUI 分支）；此前的临时 `debug` 分支已删除。
- 版本：`Cargo.toml` = `0.3.1`；tag `v0.3.1` 已正式发布（非 draft、非 prerelease）。
- 发布产物：
  - `auroku-windows-x86_64-v0.3.1.zip`
  - `auroku-linux-x86_64-v0.3.1.tar.gz`
  - `auroku-macos-universal-v0.3.1.zip`
- CI 行为：push → debug 构件（`*-v0.3.1-debug.*`）；手动 dispatch（mode=release）→ 发布。
- 最近一次 push 触发的 debug 构建：run `41728431189`，三平台全绿，release job 正确跳过。

---

## 6. 待办 / 风险

1. **设备不在场时的错误提示**：`-99` 改成明确的「设备未连接」，`wait_until` 区分“被拔掉”和“驱动没切成功”。
2. **用 Win32 API 替换外部进程**（用户已同意“构建稳定后再做”）：`powershell` → `ShellExecuteExW`；`reg query` → SetupAPI/`CM_*`；`pnputil` → `SetupDiCallClassInstaller(DIF_REMOVE)`。收益：摆脱 PowerShell 执行策略/企业策略；成本：sys crate 增加约 150 行 unsafe，只能靠 CI 验证。
3. **GUI `--help`**：GUI 子系统后终端里不再有输出；如需要可 `AttachConsole(ATTACH_PARENT_PROCESS)`。
4. **版本/分支策略**：qt6 分支仍是 `0.2.0-qt6`，合并策略见 `01`/`02`；libwdi 解耦见 `03`。
5. **PR 不构建 Windows**（`build.yml` 里 `if: github.event_name != 'pull_request'`），libwdi 相关改动无法在 PR 验证（`03` 已提出用预编译产物补门禁）。
6. **LGPL 合规**：静态链接 libwdi（LGPLv3），发布包已带 `LICENSE-LGPL-3.0.txt`，源码与构建脚本公开（满足可重链接）。

---

## 7. 复现/验证命令

```bat
:: 安装 / 恢复（需要管理员；GUI 按钮等价）
auroku --op install-driver --vid 04C5 --pid 1270
auroku --op restore-driver --vid 04C5 --pid 1270

:: 验证
auroku --op probe --vid 04C5 --pid 1270 --probe-kind seri --verbose
reg query "HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_04C5&PID_1270" /s /v Service
:: 期望 Service = WINUSB（安装后）/ usbser（恢复后）
```

本地复现构建：`AUROKU_BUILD_DEBUG=1`（或脚本参数 `--debug` / `-Debug`）走 dev profile。

---

## 8. 文件索引

| 路径 | 作用 |
| --- | --- |
| `src/driver.rs` | 驱动安装/恢复、日志、状态、提权 |
| `crates/auroku-driver-sys/` | libwdi FFI + `is_elevated` + 链接配置 |
| `src/lib.rs` | `DriverError`、`open_failure`、设备名回退 |
| `src/bin/auroku-gui/{app,worker,diag}.rs` | 驱动卡片、后台请求、诊断日志 |
| `.github/workflows/build.yml` | debug/release 双模式、libwdi 构建、CRT 校验 |
| `.github/scripts/build-*.{sh,ps1}` | 三平台构建与打包（`AUROKU_BUILD_DEBUG`） |
| `docs/windows-driver.md` | 面向用户的驱动说明 |
