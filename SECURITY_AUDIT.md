# auRokku 安全审计报告

**审计日期：** 2026-10-30
**范围：** Rust CLI 与 USB 协议逻辑、`auroku-ffi` C ABI、Qt 前端、Linux USB ACL 权限管理、构建脚本、静态编译配置及 GitHub Actions 发布工具链。

## 结论

代码审计发现两项问题，均已修复。GitHub Actions 专项审计另外发现发布构建链中仍有多个第三方 Action 使用可变标签，具有供应链风险；该项已记录，建议固定到经过审核的提交 SHA。当前 GUI 以普通用户运行，pkexec 仅用于对明确选择的 USB 节点执行 ACL 修改，不再整体提权运行 GUI。

审计未发现其他有证据支持的高置信漏洞。Rust 核心 crate 禁止 `unsafe`；FFI crate 的指针和字符串操作集中在 C ABI 边界，代码中有相应的安全契约和所有权说明。该结论不代表对所有运行时、设备固件或第三方依赖的形式化证明。

## 发现与处理

| # | 严重性 | 文件 / 行 | 问题 | 置信度 | 处理 |
|---|---|---|---|---:|---|
| 1 | HIGH | 旧版 `gui/src/platform/Elevate.cpp` | 旧实现以 root 重启 GUI，并曾转发 `QT_PLUGIN_PATH`，存在普通用户控制的 Qt 插件在 root 进程中执行的风险。 | 9/10 | 已删除整程序提权实现；当前仅对所选 USB 设备执行固定的 `setfacl` 命令。 |
| 2 | MEDIUM | [.github/workflows/build.yml](./.github/workflows/build.yml:207) | 获得 `contents: write` 权限的 Release job 使用可变 `v2` 标签引用第三方 Action，标签变更或上游账号/仓库被攻陷可能影响发布。 | 8/10 | 已固定到完整提交 SHA `3bb12739c298aeb8a4eeaf626c5b8d85266b0e65`（上游 `v2` 标签解析结果）。 |
| 3 | MEDIUM | [.github/workflows/build.yml](./.github/workflows/build.yml:43) | CI / 发布构建使用可变标签引用第三方 Actions（`dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache@v2`、`ilammy/msvc-dev-cmd@v1`、`jurplel/install-qt-action@v4`）。若上游标签被移动或 Action 被攻陷，恶意代码可在构建 runner 执行并篡改稍后发布的构建产物。 | 9/10 | 尚未修复；建议审阅上游版本后固定完整提交 SHA，并建立有计划的更新机制。 |
| 4 | LOW | `gui/src/platform/UsbAccess.cpp` | 强制终止进程或系统崩溃时，正常关闭清理流程无法运行，所选设备的临时 ACL 可能残留。 | 9/10 | 正常关闭、重新扫描和切换设备时会恢复 ACL；清理失败会阻止正常关闭并允许重试。异常终止后的残留风险已在文档中说明。 |

### Linux USB ACL 权限边界

- GUI 不会以 root 启动或重启。扫描后由用户明确选择设备，后端将选中的 bus/address 精确传给 Rust USB 枚举逻辑。
- `pkexec` 只运行固定系统路径下的 `setfacl`，目标是由 bus/address 构造并验证为字符设备的 `/dev/bus/usb/BBB/DDD`；不经 shell，不传递任意命令。
- 临时授权前保存当前用户 ACL 与 mask，并用 `setfacl -n` 避免自动扩大 mask。若 mask 不允许读写则回滚；回滚失败时保留可重试清理状态。清理时核对节点的设备号与 inode，避免将旧授权恢复到已被替换的设备节点上。
- 正常关闭、重新扫描或选择其他设备会尝试恢复原 ACL；若恢复失败，关闭操作会被拒绝并提示重试。强制终止或崩溃仍可能留下临时 ACL。

## GitHub Actions 工具链审计

- **发现：可变 Action 引用。** 除已固定 SHA 的 `softprops/action-gh-release` 外，`actions/checkout@v4`（第 41、86 行）、`dtolnay/rust-toolchain@stable`（第 43、110 行）、`Swatinem/rust-cache@v2`（第 45、114 行）、`ilammy/msvc-dev-cmd@v1`（第 88 行）、`jurplel/install-qt-action@v4`（第 90 行）、`actions/upload-artifact@v4`（第 171 行）和 `actions/download-artifact@v4`（第 186 行）仍引用可变版本标签。第三方 action 的可变标签风险列为发现 3；GitHub 官方 Actions 同样建议固定 SHA，以增强供应链完整性。
- **发布 Action pin 验证：** `softprops/action-gh-release` 的 SHA `3bb12739c298aeb8a4eeaf626c5b8d85266b0e65` 与 `v2` 标签解析结果一致。
- **权限与触发器：** 工作流默认 `contents: read`，仅 release job 请求 `contents: write`；PR 触发时跳过 build/release。手动 `workflow_dispatch` 可从所选 ref 发起发布，因此操作者应确保选择可信 ref。
- **表达式与脚本：** 手动版本输入经环境变量传入 shell，而不是直接内插脚本；用于构建命令的 matrix 参数是固定配置值。未发现可确认的命令注入问题。
- **可复现性：** Rust toolchain 使用可变 `stable`，runner 与 apt 包也不是完全锁定版本；这降低构建可复现性，但本次未将其单独定性为已证实的可利用漏洞。

## 静态编译目标

- Linux CLI：`x86_64-unknown-linux-musl`，保留静态 CRT / 静态重定位设置；构建脚本验证 ELF 静态链接、无 `PT_INTERP` 和无 `DT_NEEDED`。
- Windows CLI：保留静态 CRT 配置。
- Qt GUI：Rust FFI 后端静态链接进 GUI，但 Qt 和系统运行库仍按平台动态部署；这不是全静态 GUI。
- macOS：系统库和框架不支持完全静态链接；现有脚本及文档继续保持此限制说明。

## Qt / WrapDBus1 构建诊断

系统已安装 DBus 开发包（头文件、库及 `DBus1Config.cmake`），Qt 安装也已提供 `FindWrapDBus1.cmake`，因此无需另装一个名为 `WrapDBus1` 的系统包。失败原因是 Linux 静态 CLI 构建设置的 `PKG_CONFIG=/bin/false` 被继承到 GUI CMake 配置，且先前失败配置将 `/bin/false` 缓存在 `PKG_CONFIG_EXECUTABLE`。`build-linux.sh` 现在在 GUI 配置阶段清除该环境变量并明确指定系统 `pkg-config`，Rust FFI 构建仍在其独立构建目标中禁用 pkg-config 以维持静态链接。

## 验证

- `cargo test --workspace --locked`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `./build-linux.sh`：Linux 静态 CLI 与 Qt GUI 构建通过；静态 CLI 无 `PT_INTERP` / `DT_NEEDED`。
- `QT_QPA_PLATFORM=xcb ./dist/auroku-gui-linux-x86_64 --smoke-test`：通过。当前 Qt 环境未安装 offscreen platform plugin，因此使用可用的 XCB 插件验证。
- 未在真实 USB 设备上执行授权集成测试；测试环境没有可安全修改 ACL 的 USB 设备节点。
- GitHub Actions 工作流引用、触发器、权限、发布产物流转及 shell 输入路径已完成人工专项审计；未执行 GitHub 远端 workflow。
