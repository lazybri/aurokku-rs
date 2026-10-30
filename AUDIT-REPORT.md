# 代码与安全审计报告

- 审计日期：2026-10-30
- 范围：`src/` 中的 Rust 代码、`Cargo.toml`、Cargo 配置、各平台构建脚本及中英文 README
- 目标：检查安全风险，并确认静态编译的实现与验证状态

## 结论

此前审计发现的 Linux GUI 整体提权问题已通过受限权限辅助模式缓解。GUI 不会以 root 运行；用户扫描并选择设备后，才通过 polkit 启动一次性辅助模式。辅助模式重新核对 VID/PID/总线号/设备地址，仅对对应 `/dev/bus/usb/BBB/DDD` 节点增加当前用户读写 ACL，然后立即退出。GUI、配置文件、网络和显示服务不获得 root 权限；USB 任务也绑定所选总线号和设备地址。GUI 检测到有效 UID 为 root 时会拒绝启动。

Linux CLI 已通过 Windows 上的 Zig 交叉构建成功生成，并确认是无动态解释器、无动态库依赖的 ELF。Windows 和 macOS 的 CLI / GUI 构建命令已合并为一次 Cargo 调用；Linux CLI 与 GUI 仍分别构建，以保留静态 CLI 和兼容图形运行时的 GUI。

## 安全审查

| # | Severity | File | Lines | Vulnerability | Confidence |
|---|----------|------|-------|---------------|------------|
| 1 | 🟠 HIGH（已缓解） | `src/bin/auroku-gui/elevate.rs`（旧文件，已删除） | 历史实现 | 旧实现会把整个 GUI 进程以 root 重新启动。该路径已移除；当前 GUI 以普通用户运行，权限 helper 仅向确认过的所选 USB 节点添加用户 ACL。针对本次按设备授权改动的复审未发现安全漏洞。 | 8/10 |

审查覆盖 Rust 源码、Cargo manifests/lockfile 与配置、GitHub Actions、各平台构建脚本、README 和先前审计报告。Rust crate 使用 `#![forbid(unsafe_code)]`。Linux GUI 不再以 root 执行 UI，也不转发环境变量；权限 helper 通过无 shell 的参数调用并校验目标设备身份，只修改对应 USB 节点 ACL。需要系统安装 polkit 和 `getfacl`/`setfacl`（acl 软件包）。

**ACL 生命周期：** 正常关闭 GUI 时，会通过一次 polkit 操作恢复本次 GUI 运行中所有已授权节点的原有用户 ACL，或移除原先不存在的 ACL 条目。若用户取消关闭时的 polkit 授权，或 GUI 被强制终止/崩溃，ACL 可能保留至设备节点移除。helper 在撤销前重新核对设备 VID/PID/总线号/地址，并仅修改当前用户对应的 ACL 条目；若 ACL 已被其他进程更改，则保留现状并报告错误。

## 静态编译状态

- **Linux CLI**：目标为 `x86_64-unknown-linux-musl`，配置了静态 CRT 和静态 relocation；Linux 构建脚本还会检查 ELF 是否包含动态解释器或动态依赖。
- **Windows CLI/GUI**：配置了 `crt-static`；Windows 系统 DLL 仍由操作系统提供。
- **Linux GUI**：使用宿主 glibc 并动态加载 Wayland/X11/OpenGL，不适合构建为可运行的全静态 musl 程序。
- **macOS**：系统 `libSystem` 与框架必须动态链接。

## 验证

- `cargo test --locked`：通过，16 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo test --all-targets --all-features --offline`：Windows 主机下通过，17 个测试通过。
- `cargo clippy --all-targets --all-features --offline -- -D warnings`：通过。
- Linux GUI musl 目标 `cargo check --tests --features gui --bin auroku-gui`：通过；权限 helper 与 Linux 测试代码均通过类型检查。WSL2 缺少 Linux Rust 工具链，未运行 Linux 测试。
- `cargo build --release --locked --offline --features gui --bins`：Windows GNU 主机下通过，验证合并后的 CLI/GUI 构建调用，且两个产物均生成。
- Windows → Linux musl 交叉构建：使用 Zig 运行 `windows-cross-build-linux.ps1` 成功；本次更改后重新构建并验证。
- Linux ELF 静态性：检查构建产物为 ELF64，未发现 `PT_INTERP` 或 `DT_NEEDED`，确认无动态加载器和动态库依赖。
- WSL2：发现 Arch Linux 发行版，但未安装 Linux `cargo`、`rustc` 或 `musl-gcc`，因此没有在 WSL 内运行 `build-linux.sh`；静态 CLI 已通过上述 Zig 交叉构建验证。
- Windows PowerShell 构建脚本语法检查：通过。
- macOS Bash 构建脚本语法检查：通过（对 Windows 工作区的 CRLF 行尾作标准化后检查）。

## 后续建议

1. 在 Linux 环境实际使用 polkit 和 USB 设备，验证授权、取消授权及拔出设备后 ACL 生命周期；并运行 `build-linux.sh` 验证宿主机静态 CLI/动态 GUI 构建。
