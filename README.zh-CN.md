[English](README.md) | **简体中文**

# auRokku (Rust)

`auRokku.py` 的 Rust 重写版本，协议行为、Type2Auth 认证算法、JSON 结构与
命令行接口保持一致。感谢 Yuu 所提供的 Python 版源码。

## 功能

- UIM 锁读取 / 写入 / 清除（0x54 / 0x72 / 0x57）
- 手机锁码（PIN，NV 82）读取 / 重置（0xDC 0x00 / 0xDC 0x01）
- NV 10481 维护模式位（0xE1 + auth 4000）
- ICCID 记录读取（0x59）
- 单会话综合状态 / 批量查询
- USB 布局枚举、SERI 回声与 SCDP 主动探测（`--op probe`）
- Type2Auth 52/53 认证（`--type2`）
- 可选 JSON 输出、详细 USB 帧日志
- Qt 6 Widgets GUI（原生 Qt 风格，中英文界面，浅色 / 深色主题），通过 C ABI 调用同一 Rust 后端

## GUI 前端（Qt 6 / Widgets）

`auroku-gui` 是用 C++ / Qt 6 Widgets 重写的桌面前端，通过 `crates/auroku-ffi`
提供的 C ABI 直接调用原有 Rust 后端。界面以标准 Qt 控件为基础，仅在其上叠加
一层轻量配色（导航栏、卡片、按钮、开关、分段按钮、状态 chip、Toast 与确认
对话框），提供浅色 / 深色两套配色，并支持中英文 UI。

- **架构**：`crates/auroku-ffi` 将 `auroku_scan_devices` 与 `auroku_run_task`
  通过静态 C ABI 库（`libauroku_ffi.a` / `auroku_ffi.lib`）直接链接进 GUI
  可执行文件。请求与响应都使用 JSON 信封，C++ 侧无需建模 Rust 类型。所有 USB
  调用在独立 `QThread` 上执行，界面不会因 libusb 传输或超时而卡住。
- **界面语言**：可在导航栏的语言选择器中切换简体中文 / English，或用
  `--lang=en|zh` 指定。首次启动根据系统区域自动选择（`zh*` 为中文，其他为
  英文）。若系统没有可用的 CJK 字体，会自动回退到 English。
- **主题**：导航栏的月亮 / 太阳按钮可切换浅色与深色；默认以深色启动，`--light` 可改用浅色启动。

构建依赖：Qt 6.5+、CMake 3.21+、Ninja 以及 Rust 工具链。CMake 会自动构建
Rust C ABI 库。

```bash
# 配置与构建（Linux / Windows / macOS 相同命令）
cmake -S gui -B build/gui -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build/gui

# 运行
./build/gui/auroku-gui
```

Windows 下通过 `-DCMAKE_PREFIX_PATH=C:/Qt/6.8.3/mingw_64`（或 `msvc2022_64`）
指定 Qt；`build-windows.ps1` 会自动查找 Qt 并把运行时打包到可执行文件旁。

开发参数：`--smoke-test`（启动后退出）、`--screenshot=FILE`、`--page=N`（0–6）
可用于 `QT_QPA_PLATFORM=offscreen` 的无界面检查。

平台说明：

- **Linux**：需要 Qt 6 运行时（`qt6-base`）；原生 Wayland 会话还需安装
  `qt6-wayland`（否则 Qt 会使用 XWayland）。访问 USB 设备通常需要 root 或对应
  udev 规则。GUI 是普通的动态链接 Qt 应用；Rust 后端已静态链接进可执行文件，
  因此无法做成全静态。
- **Windows**：`windeployqt` 会把 Qt DLL 与插件部署到 `auroku-gui.exe` 旁边，
  无需单独安装 Qt。
- **macOS**：`build-macos.sh` 产出 `dist/auroku-gui.app`（本机架构），并在可用时
  调用 `macdeployqt`。
- **字体**：启动时自动查找系统 CJK 字体（微软雅黑 / Noto Sans CJK / 苹方等）。
  可通过环境变量 `AUROKU_GUI_FONT=/path/to/font.ttf` 指定字体文件或字体族。
- **权限**：与 CLI 相同；Windows 可能仍需安装 libusb 驱动。

### Linux polkit 提权

Linux 上 GUI 启动时会默认调用 `pkexec`（polkit）请求 root 权限，认证成功后以
root 重新执行自身，从而直接访问 USB 设备：

- 跳过提权：启动参数 `--no-polkit`，或环境变量 `AUROKU_GUI_NO_POLKIT=1`。
- 认证取消、polkit 不可用或 `pkexec` 缺失：程序会以普通用户继续运行，概览页
  显示权限警告；USB 操作可能因权限不足而失败。
- 提权时只转发显示 / 会话 / 语言相关环境变量（Wayland、X11、DBus、locale），
  不会转发 `LD_*`、`PATH`、`HOME` 等敏感变量；世界可写的可执行文件会被拒绝提权。
- 部分 Wayland 合成器可能限制 root 客户端。如果 root 模式无法连接显示，可尝试
  `QT_QPA_PLATFORM=xcb`（需要 XWayland），或配置 udev 规则并以普通用户运行。
- 纯 Wayland 会话（未设置 `DISPLAY`）下会自动跳过提权，因为 root 客户端通常
  无法连接合成器。请使用 udev 规则，或设置 `QT_QPA_PLATFORM=xcb` 通过 XWayland
  提权。
- 安全提示：整进程以 root 运行会扩大攻击面。若不需要自动提权，可跳过 polkit
  并仅为设备配置 udev 规则。

GUI 中的“概览 / UIM 锁 / PIN & ICCID / 探测 / 批量 / 会话 / 控制台”页面覆盖了
CLI 的全部 `--op` 操作，并在控制台页面显示 USB TX / RX 日志和完整 JSON 结果。

## 构建

需要 Rust 工具链（<https://rustup.rs>）。`rusb` 使用 vendored libusb，
因此还需要一个 C 编译器（Linux/macOS 的 `gcc`/`clang`，Windows 的 MSVC
Build Tools 或 MinGW-w64）。Qt GUI 还需要 Qt 6.5+、CMake 3.21+ 与 Ninja。

各平台提供本地编译脚本，产物统一放到 `dist/`。**Linux、Windows、macOS 三个
原生脚本都会同时构建 CLI 与 GUI**。这里的“静态”指 Rust/C 运行时依赖；系统库
与图形栈是否可静态链接由操作系统决定：

| 平台 | 命令 | 产物 |
| --- | --- | --- |
| Linux（静态 CLI + Qt 6 GUI） | `chmod +x build-linux.sh && ./build-linux.sh` | `dist/auroku-linux-x86_64`（静态 ELF）、`dist/auroku-gui-linux-x86_64`（Qt 6） |
| Windows（静态 CRT） | `.\build-windows.ps1` | `dist\auroku.exe`、`dist\auroku-gui-windows-<arch>\auroku-gui.exe`（含 Qt 运行时）、`dist\auroku-windows-<arch>.zip` |
| macOS（通用 CLI + 本机 GUI） | `chmod +x build-macos.sh && ./build-macos.sh` | `dist/auroku`（arm64 + x86_64）、`dist/auroku-gui.app` |
| macOS（仅本机架构） | `./build-macos.sh --native` | `dist/auroku`、`dist/auroku-gui.app` |

静态链接结论：

| 产物 | 静态状态 |
| --- | --- |
| Linux CLI (`auroku`) | **完全静态**：musl + 静态 CRT，无 `DT_NEEDED`、无 `PT_INTERP` |
| Windows CLI (`auroku.exe`) | 静态 CRT（`crt-static`）；`kernel32`/`user32` 等系统 DLL 由 Windows 提供，无法静态 |
| macOS CLI | libSystem 与系统框架必须动态链接（macOS 限制）；Rust 依赖与 libusb 静态 |
| Linux GUI (`auroku-gui-linux-x86_64`) | 动态链接 Qt 6；Rust 后端已链接进可执行文件；**不能**全静态 |
| Windows GUI (`auroku-gui.exe`) | Qt 6 DLL 与插件随可执行文件一起分发 |
| macOS GUI (`auroku-gui.app`) | Qt 6 框架由 `macdeployqt` 放入 app bundle |

Linux GUI 是普通的动态链接 Qt 6 应用；Rust 后端已链接进可执行文件。请安装
Qt 6 运行时（`qt6-base`），或将 Qt 库与二进制一起分发。

### 静态链接验证

Linux CLI 静态产物可用以下命令复核（本项目 x86_64 环境已实测通过）：

```bash
file dist/auroku-linux-x86_64
# ELF 64-bit LSB executable ... statically linked
readelf -d dist/auroku-linux-x86_64 | grep NEEDED   # 无输出
readelf -l dist/auroku-linux-x86_64 | grep INTERP   # 无输出
ldd dist/auroku-linux-x86_64                        # not a dynamic executable
```

`build-linux.sh` 在构建结束后会自动执行上述静态性检查，失败时直接退出。
Rust 后端也已链接进 GUI，因此 GUI 不应再引用旧的共享库：

```bash
readelf -d dist/auroku-gui-linux-x86_64 | grep libauroku_ffi   # 无输出
```

Windows CLI（GNU 构建；MSVC 构建同样静态链接 CRT）：

```powershell
objdump -p dist\auroku.exe | Select-String 'DLL Name'
# 预期仅有 KERNEL32.dll、msvcrt.dll、ntdll.dll、bcryptprimitives.dll 与
# api-ms-win-core-synch-*.dll —— 这些都是 Windows 自带系统 DLL。
```

`build-windows.ps1` 会自动执行该检查，一旦 CLI 意外依赖 MinGW / Qt / FFI DLL
就会直接失败。Qt GUI 有意**不是**全静态的：它在运行时加载 Qt 6，而 Rust 后端、
libusb 及其依赖都已直接链接进 `auroku-gui.exe`。

Linux 构建需要 `musl-gcc` 与内核 UAPI 头文件：

```bash
# Debian/Ubuntu
sudo apt-get install musl-tools linux-libc-dev
# Fedora
sudo dnf install musl-gcc kernel-headers
# Arch
sudo pacman -S musl linux-api-headers
```

也可以直接使用 Cargo。`.cargo/config.toml` 已为 musl 与 Windows 目标启用静态
链接；Linux 上脚本会额外准备内核 UAPI 头文件。直接构建时需要禁用 pkg-config，
避免把宿主 glibc 的 `libudev` 链入静态二进制：

```bash
PKG_CONFIG=/bin/false \
CC_x86_64_unknown_linux_musl=musl-gcc \
AR_x86_64_unknown_linux_musl=ar \
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
C_INCLUDE_PATH=/usr/include:/usr/include/x86_64-linux-gnu \
  cargo build --release --target x86_64-unknown-linux-musl --bin auroku
```

产物：`target/x86_64-unknown-linux-musl/release/auroku`（静态 ELF）。推荐直接
使用 `build-linux.sh`，它会准备 UAPI 头文件并自动校验静态性。

### 交叉编译 Linux（x86_64 静态 musl）

Windows 上可用 Zig 交叉编译，无需 WSL / Docker：

```powershell
# 一次性准备：安装 Zig，解压到 %USERPROFILE%\tools\zig-<版本>，
# 或者设置 ZIG 环境变量 / 用 -ZigPath 指定 zig.exe
.\windows-cross-build-linux.ps1
```

产物：`target\x86_64-unknown-linux-musl\release\auroku`
（x86_64 静态 ELF，无运行时依赖；访问 USB 设备需要 root 或对应 udev 规则）

该交叉编译脚本只产出静态 CLI：Qt GUI 需要完整的 Linux Qt 6 sysroot 与 C++
工具链。请在 Linux 上运行 `./build-linux.sh` 构建 GUI，或直接下载 Release
中的 Linux 包。

### 构建 macOS 通用二进制（arm64 + x86_64）

需要在 macOS 上执行（安装 Xcode Command Line Tools）：

```bash
chmod +x build-macos.sh
./build-macos.sh
```

脚本会分别为 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 编译 CLI，再用
`lipo` 合并并做 ad-hoc 签名；随后为当前机器架构构建 Qt 6 GUI bundle。

产物：`dist/auroku`（通用 CLI）、`dist/auroku-gui.app`（本机架构 Qt GUI）

仅构建当前机器架构可使用 `./build-macos.sh --native`（速度更快）。

> macOS 不支持完全静态的可执行文件：libSystem 与系统框架必须动态链接。
> 脚本已将全部 Rust 依赖与 vendored libusb 静态链接，除此之外没有可用的
> 静态编译选项。

## GitHub Actions

工作流位于 `.github/workflows/build.yml`，一次触发即可完成
**测试 → 三平台构建 → 发布 Release**：

- **Pull request**：只运行 `cargo fmt --check`、`cargo clippy -D warnings`
  与 `cargo test`。
- **手动触发（workflow_dispatch）**：填写版本号（如 `v0.1.0`，省略 `v`
  会自动补全），可选预发布 / 草稿。工作流会先跑测试，再并行构建
  Windows、Linux（musl 静态）与 macOS 通用二进制，最后自动创建
  GitHub Release 并上传全部产物。
- **推送 `v*` 标签**：同样在一次运行内完成测试、构建与发布。

产物（每个包内均包含 CLI 与 Qt 6 GUI）：

- `auroku-windows-x86_64.zip`（静态 CRT CLI + 含 Qt 运行时的 GUI 目录）
- `auroku-linux-x86_64.tar.gz`（musl 静态 CLI + Qt 6 GUI）
- `auroku-macos-universal.zip`（通用 CLI + `auroku-gui.app`）

Linux GUI 在 `ubuntu-22.04` 上针对工作流安装的 Qt 6.8 SDK 构建；目标机需要
安装 Qt 6 运行时（或自行打包 Qt 库）。同一包内的 CLI 仍然是完全静态的。

> Windows / Linux 主机无法直接交叉编译 macOS 二进制：`rusb` 的 vendored
> libusb 需要 Apple SDK 中的 IOKit / CoreFoundation 头文件与链接存根，
> 而该 SDK 不可再分发。因此 macOS 版本由 Actions 的 `macos-14` runner 构建。

协议层纯函数附带单元测试（nibble 编解码、SN 解析、密钥派生、认证包向量等）：

```bash
cargo test
```

## 使用示例

`--op`、`--vid`、`--pid` 为必填参数：

```bash
# 枚举 USB 配置 / 接口 / bulk 端点对
auroku --op probe --vid 0482 --pid 0a5c

# 主动探测 SERI 回声 / SCDP
auroku --op probe --vid 0482 --pid 0a5c --probe-kind seri
auroku --op probe --vid 0482 --pid 0a5c --probe-kind scdp

# 综合状态
auroku --op status --vid 0482 --pid 0a5c

# UIM 锁
auroku --op readuimlk --vid 0482 --pid 0a5c
auroku --op clearlk --vid 0482 --pid 0a5c
auroku --op writelk --vid 0482 --pid 0a5c --val 80e48ed2 --control 1

# 手机锁码
auroku --op readpin --vid 0482 --pid 0a5c
auroku --op resetpin --vid 0482 --pid 0a5c

# 维护位
auroku --op maintenance_bit --vid 0482 --pid 0a5c --control 1

# 批量查询 / 退出
auroku --op batch --vid 0482 --pid 0a5c --profile uimlk
auroku --op exit --vid 0482 --pid 0a5c
auroku --op exituimlk --vid 0482 --pid 0a5c

# JSON 输出 / 详细日志 / 手动指定 SN / Type2Auth 认证
auroku --op status --vid 0482 --pid 0a5c --json
auroku --op status --vid 0482 --pid 0a5c --verbose
auroku --op status --vid 0482 --pid 0a5c --sn SKYKA077394
auroku --op status --vid 0482 --pid 0a5c --sn 947307414B594B53 --type2
```

## 与 Python 版本的对应关系

| Python | Rust |
| --- | --- |
| `encode_nibbles` / `decode_nibbles` | 同名函数 |
| `decode_lockcode_digits` | 同名函数 |
| `parse_sn_to_type2auth` | 同名函数（返回 `(Vec<u8>, String)`） |
| `parse_usb_endpoint` | 同名函数（作为 clap `value_parser`） |
| `bitrev8` / `derive_type2auth_key` | 同名函数 |
| `compute_type2auth_request` | 同名函数 |
| `AuRokkuDevice` | 同名 `struct`（`rusb` 实现传输层） |
| `open_device` / `connect` / `close` | 同名方法 |
| `inspect_usb` / `probe` / `_read_probe_response` | 同名方法 |
| `authenticate_step` / `enter_base` / `enter_uimlk` | 同名方法 |
| `print_human_report` | 同名函数 |

实现细节：

- Rust 版通过端点描述符自动选择 Bulk / Interrupt 传输类型，与 pyusb
  `read` / `write` 的自动判定行为一致。
- `--sn` 为 16 位十六进制时按 Python 语义原样作为 Type2Auth 材料，不再
  反推 11 位序列号。
- 错误类型名（`ValueError` / `RuntimeError` / `TimeoutError`）与退出码
  （失败为 1）均与 Python 版保持一致。

