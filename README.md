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

## 构建

需要 Rust 工具链（<https://rustup.rs>）。`rusb` 使用 vendored libusb，
因此还需要一个 C 编译器（Linux/macOS 的 `gcc`/`clang`，Windows 的 MSVC
Build Tools 或 MinGW-w64）。

各平台提供本地编译脚本，产物统一放到 `dist/`：

| 平台 | 命令 | 产物 |
| --- | --- | --- |
| Linux | `chmod +x build-linux.sh && ./build-linux.sh` | `dist/auroku-linux-x86_64` |
| Linux（静态 musl） | `./build-linux.sh --musl` | 同上（静态 ELF，无运行时依赖） |
| Windows | `.\build-windows.ps1` | `dist\auroku.exe`、`dist\auroku-windows-<arch>.zip` |
| macOS（通用二进制） | `chmod +x build-macos.sh && ./build-macos.sh` | `dist/auroku`（arm64 + x86_64） |
| macOS（仅本机架构） | `./build-macos.sh --native` | `dist/auroku` |

也可以直接使用 Cargo：

```bash
cargo build --release
```

产物：`target/release/auroku`（Windows 为 `auroku.exe`）。

### 交叉编译 Linux（x86_64 静态 musl）

Windows 上可用 Zig 交叉编译，无需 WSL / Docker：

```powershell
# 一次性准备：安装 Zig，解压到 %USERPROFILE%\tools\zig-<版本>，
# 或者设置 ZIG 环境变量 / 用 -ZigPath 指定 zig.exe
.\windows-cross-build-linux.ps1
```

产物：`target\x86_64-unknown-linux-musl\release\auroku`
（x86_64 静态 ELF，无运行时依赖；访问 USB 设备需要 root 或对应 udev 规则）

### 构建 macOS 通用二进制（arm64 + x86_64）

需要在 macOS 上执行（安装 Xcode Command Line Tools）：

```bash
chmod +x build-macos.sh
./build-macos.sh
```

脚本会分别编译 `aarch64-apple-darwin` 和 `x86_64-apple-darwin`，再用
`lipo` 合并，并对结果做 ad-hoc 签名：

产物：`dist/auroku`（同时支持 Apple Silicon 与 Intel Mac）

仅构建当前机器架构可使用 `./build-macos.sh --native`（速度更快）。

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

产物：

- `auroku-windows-x86_64.zip`
- `auroku-linux-x86_64.tar.gz`（musl 静态 ELF）
- `auroku-macos-universal.zip`（arm64 + x86_64）

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

