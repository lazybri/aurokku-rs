# 代码与安全审计报告

- 审计日期：2026-10-30
- 范围：`src/` 中的 Rust 代码、`Cargo.toml`、Cargo 配置、各平台构建脚本及中英文 README
- 目标：检查安全风险，并确认静态编译的实现与验证状态

## 结论

此前审计发现的 Linux GUI 整体提权问题已通过受限权限辅助模式缓解。GUI 不会以 root 运行；用户扫描并选择设备后，才通过 polkit 启动一次性辅助模式。辅助模式重新核对 VID/PID/总线号/设备地址，仅对对应 `/dev/bus/usb/BBB/DDD` 节点增加当前用户读写 ACL，然后立即退出。GUI、配置文件、网络和显示服务不获得 root 权限；USB 任务也绑定所选总线号和设备地址。GUI 检测到有效 UID 为 root 时会拒绝启动。

Linux CLI 已通过 Windows 上的 Zig 交叉构建成功生成，并确认是无动态解释器、无动态库依赖的 ELF；2026-10-30 又在真实 Linux 主机上用 `build-linux.sh` 本机构建并复验了同一结论（见「Linux 本地复检」）。Windows 和 macOS 的 CLI / GUI 构建命令已合并为一次 Cargo 调用；Linux CLI 与 GUI 仍分别构建，以保留静态 CLI 和兼容图形运行时的 GUI。

本次 Linux 复检修复了 1 处不影响运行行为的问题：GUI 代码在 clippy 1.99 下触发 2 个 lint，导致 `cargo clippy --all-features -D warnings` 失败。

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

以下命令已于 2026-10-30 在 Linux 主机（CachyOS，kernel 7.3，rustc/clippy 1.99.1，rustup，已安装 musl / acl / polkit）上重新执行，结论与此前 Windows 主机的记录一致：

- `cargo test --locked`：通过，16 个测试通过。
- `cargo fmt --all -- --check`：通过。
- `cargo test --all-targets --all-features --offline`：Linux 主机下通过，18 个测试通过（16 个库测试 + GUI 的 `i18n`、`permissions` 各 1 个）。
- `cargo clippy --all-targets --all-features --offline -- -D warnings`：修复 2 个 lint 后通过（详见「Linux 本地复检」）。
- `cargo clippy --all-targets --offline -- -D warnings`（CI 实际用法）：通过。
- Linux GUI musl 目标 `cargo check --tests --features gui --bin auroku-gui --target x86_64-unknown-linux-musl`：通过；权限 helper 与 Linux 测试代码均通过类型检查。
- `cargo build --release --locked --offline --features gui --bins`：通过，验证合并后的 CLI/GUI 构建调用，且两个产物均生成。
- `./build-linux.sh`：在真实 Linux 主机上完整通过，生成静态 CLI 与宿主机 glibc GUI。
- Windows → Linux musl 交叉构建：使用 Zig 运行 `windows-cross-build-linux.ps1` 成功；本次更改后重新构建并验证。
- Linux ELF 静态性：静态 CLI 为 ELF64 且无 `PT_INTERP`、无 `DT_NEEDED`，`ldd` 报“不是动态可执行文件”；GUI 仅依赖 `libgcc_s.so.1`、`libm.so.6`、`libc.so.6` 与 `ld-linux-x86-64.so.2`，`ldd` 全部解析成功，未链接 libudev。
- Windows PowerShell 构建脚本语法检查：通过。
- macOS Bash 构建脚本语法检查：通过（对 Windows 工作区的 CRLF 行尾作标准化后检查）。

## Linux 本地复检（2026-10-30）

环境：CachyOS（kernel 7.3）、rustc 1.99.1 / clippy 1.99.1、rustup（已安装 `x86_64-unknown-linux-musl`）、`musl 1.2.6`、`acl 2.4.0`、`polkit 128`。

| # | 检查项 | 上次报告（Windows 主机 / 交叉构建） | 本次 Linux 主机 |
|---|--------|----------------------------------------|-----------------|
| 1 | `cargo fmt --all -- --check` | 通过 | ✅ 通过 |
| 2 | `cargo test --locked` | 16 passed | ✅ 16 passed |
| 3 | `cargo test --all-targets --all-features --offline` | 17 passed | ✅ 18 passed（含 2 个 GUI 测试） |
| 4 | `cargo clippy --all-targets --all-features --offline -- -D warnings` | 通过 | ⚠️ 初始失败（2 个 lint，已修复）→ ✅ 通过 |
| 5 | `cargo clippy --all-targets -- -D warnings`（CI 形式） | — | ✅ 通过（修复前后均通过） |
| 6 | GUI musl 目标 `cargo check --tests --features gui` | 通过（未跑 Linux 测试） | ✅ 通过（无需 WSL） |
| 7 | `cargo build --release --features gui --bins` | 通过 | ✅ 通过 |
| 8 | `./build-linux.sh` | 未在真实 Linux 运行 | ✅ 通过 |
| 9 | 静态 CLI 无 `PT_INTERP` / `DT_NEEDED` | 通过（Zig 交叉构建产物） | ✅ 通过（本机构建产物） |
| 10 | GUI 动态依赖最小化 | 仅系统图形库 | ✅ 仅 glibc 系 4 个库，无 libudev |

### 安全性复核（可自动验证部分）

- 三个 crate 根均为 `#![forbid(unsafe_code)]`（`src/lib.rs`、`src/main.rs`、`src/bin/auroku-gui/main.rs`），全仓库无 `unsafe` 用法。
- root 拒启动：以用户命名空间 root（`unshare -r`）运行 GUI，程序输出拒绝信息并以退出码 1 结束，未进入窗口初始化。
- 权限 helper：非 root 直接运行 `--grant-usb-access` / `--revoke-usb-access` 均被拒绝（缺少 `PKEXEC_UID`，退出码 1）；参数解析严格，ACL 状态仅接受 `r`/`w`/`x`/`-` 组成的三字符权限。
- 受信可执行检查：本机 `/usr/bin/{pkexec,setfacl,getfacl}` 均为 root:root 且无组/其他写位；`/`、`/home`、`/home/zsyizh`（0700）及项目目录均满足“属主为当前用户或 root、无组/其他写位”的祖先目录条件，`trusted_executable` 在本机可通过。
- ACL 生命周期语义：在本机 acl 2.4.0 上实测，`setfacl -x u:<uid>` 后 `mask::` 条目仍会保留，`restore_acl_snapshot` 通过随后的 `setfacl -x m` 将 0660 / 0640 文件精确还原为原始 ACL，与实现逻辑一致。
- 本次复核未发现新的安全漏洞；未改动 pkexec / ACL 实现。

### 复检中修复的问题

- `cargo clippy --all-targets --all-features -- -D warnings` 在 clippy 1.99 下失败：`src/bin/auroku-gui/permissions.rs` 触发 `clippy::while_let_loop`（revoke 参数循环）与 `clippy::byte_char_slices`（ACL 权限校验）。已改写为 `while let` 与 `*b"rwx"`，行为不变，修复后该命令通过。
- 该问题此前未暴露，原因是 CI 的 lint/test 步骤使用 `cargo clippy --all-targets -- -D warnings` 与 `cargo test --all-targets`，不带 `--features gui`；GUI 代码（含其单测）从未进入 CI 的 clippy 与测试。

## 后续建议

1. 在 Linux 环境实际使用 polkit 和 USB 设备，验证授权、取消授权及拔出设备后 ACL 生命周期（本次环境无可用 SERI 设备，且 `pkexec` 需要交互授权，无法无人值守验证）；宿主机 `build-linux.sh` 静态 CLI / 动态 GUI 构建已在本机验证通过。
2. 在 CI 的 lint/test 步骤加入 `--all-features`（或单独一条 GUI job），使 GUI 代码参与 clippy 与测试，避免同类问题再次漏检。
