# kddilck (Rust)

`kddilck.py` 的 Rust 重写版本，协议行为、认证算法与命令行接口保持一致。
感谢Yuu所提供的python版源码

## 功能

- UIM 锁读取 / 写入 / 清除（0x54 / 0x72 / 0x57）
- 手机锁码（PIN，NV 82）读取 / 重置（0xDC 0x00 / 0xDC 0x01）
- NV 10481 维护模式位（0xE1 + auth 4000）
- ICCID 记录读取（0x59）
- 单会话综合状态 / 批量查询
- 可选 JSON 输出、详细 USB 帧日志

## 构建

需要：

- Rust 工具链（<https://rustup.rs>）。Windows 上使用 GNU 目标：
  `rustup default stable-x86_64-pc-windows-gnu`
- MinGW-w64 GCC（`rusb` 的 vendored libusb 需要 C 编译器）。
  Windows 可通过 MSYS2 安装 `mingw-w64-x86_64-gcc`，并把
  `C:\msys64\mingw64\bin` 加入 `PATH`；Linux 使用发行版的 `gcc`。

```powershell
cd f:\proj\kddilck
cargo build --release
```

产物：`target\release\kddilck.exe`（仅依赖 Windows 系统 DLL，无需额外运行时）

### 交叉编译 Linux（x86_64 静态 musl）

Windows 上可用 Zig 交叉编译，无需 WSL / Docker：

```powershell
# 一次性准备：安装 Zig，解压到 %USERPROFILE%\tools\zig-<版本>，
# 或者设置 ZIG 环境变量 / 用 -ZigPath 指定 zig.exe
.\build-linux.ps1
```

脚本会自动安装 `x86_64-unknown-linux-musl` 目标，并生成 Zig 包装器用于链接
和编译 vendored libusb。

产物：`target\x86_64-unknown-linux-musl\release\kddilck`
（x86_64 静态 ELF，无运行时依赖；访问 USB 设备需要 root 或对应 udev 规则）

### 构建 macOS 通用二进制（arm64 + x86_64）

需要在 macOS 上执行（安装 Xcode Command Line Tools）：

```bash
chmod +x build-macos.sh
./build-macos.sh
```

脚本会分别编译 `aarch64-apple-darwin` 和 `x86_64-apple-darwin`，再用
`lipo` 合并，并对结果做 ad-hoc 签名：

产物：`dist/kddilck`（同时支持 Apple Silicon 与 Intel Mac）

> Windows / Linux 主机无法直接交叉编译 macOS 二进制：`rusb` 的 vendored
> libusb 需要 Apple SDK 中的 IOKit / CoreFoundation 头文件与链接存根，
> 而该 SDK 不可再分发。没有 Mac 时可以使用仓库自带的 GitHub Actions
> 工作流 `.github/workflows/build.yml`：在 Actions 中手动运行 **Build binaries**，
> 通过 `target` 下拉框单选 `windows-x86_64`、`linux-x86_64` 或
> `macos-universal`，产物以同名 artifact 上传。

协议层纯函数附带单元测试（nibble 编解码、SN 解析、密钥派生、认证包布局等）：

```powershell
cargo test
```

## 使用示例

```powershell
kddilck --op status
kddilck --op readuimlk
kddilck --op clearlk
kddilck --op writelk --val 80e48ed2 --control 1
kddilck --op readpin
kddilck --op resetpin
kddilck --op write10481 --control 1
kddilck --op exit

# JSON 输出 / 详细日志 / 手动指定 SN
kddilck --op status --json
kddilck --op status -v
kddilck --op status --sn SKYKA077394
```

## 与 Python 版本的对应关系

| Python | Rust |
| --- | --- |
| `encode_nibbles` / `decode_nibbles` | 同名函数 |
| `decode_lockcode_digits` | 同名函数 |
| `parse_sn_to_nv10077` | 同名函数（返回 `(Vec<u8>, String)`） |
| `bitrev8` / `derive_k` | 同名函数 |
| `compute_auth_request` | 同名函数 |
| `KddiSeriDevice` | 同名 `struct`（`rusb` 实现传输层） |
| `save_operation_log` | 原始 Python 版本中已是空桩，未移植 |

注意：Rust 版通过端点描述符自动选择 Bulk / Interrupt 传输类型，与 pyusb `read` / `write`
的自动判定行为一致。
