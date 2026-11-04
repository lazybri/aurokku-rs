[English](README.md) | **简体中文**

# auRokku · RP2354 固件分支（`rp2354`）

本分支把 auRokku 转为 **RP2354A 手持固件** 项目。这里是研究/固件分支：按用户
定义，桌面端（CLI + egui/Qt GUI、Windows 驱动栈、发布流水线）已从本分支移除，
继续保留在 `main` / `qt6` 分支。协议代码移植自 auRokku v0.3.2（`main` @ `ee37a79`）。

本分支采用 **mock 优先**：USB host 栈与 OLED 面板之下的全部逻辑都是普通 Rust
库，可在主机上用脚本化手机跑 `cargo test`，不依赖任何硬件。这样在自制 PCB
（docs/plan/06 §7，G1 门禁）出现之前就能开发与评审固件逻辑。

## 目录结构

```text
crates/auroku-core/          # no_std + alloc 协议核心（与桌面共享）
  src/codec.rs               #   nibble/lockcode/hex 编解码
  src/type2auth.rs           #   MD5 Type2Auth 0x52/0x53
  src/frame.rs               #   帧缓冲、超时、0xAF 过滤
  src/session.rs             #   base/UIMLK 会话与认证
  src/ops.rs                 #   12 个 Task 与强类型报告
  src/mock.rs                #   脚本化 request/response UsbTransport
firmware/auroku-firmware/    # RP2354 固件逻辑（可在主机测试）
  src/hal.rs                 #   Display / Input / Clock trait
  src/ui.rs                  #   OLED 菜单状态机（docs/plan/09）
  src/protocol.rs            #   auroku-core 会话封装
  src/app.rs                 #   输入 -> UI -> 协议 -> 显示 事件循环
  src/mock.rs                #   MockDisplay / MockInput / MockClock
spike/auroku-core-nostd/     # 早期 no_std 可行性 spike（docs/plan/10）
docs/plan/                   # RP2354 研究与设计文档（06-10、13）
```

## 状态

| 部分 | 状态 |
|---|---|
| 协议核心（`crates/auroku-core`） | 已从桌面 v0.3.2 移植；12 个 Task；mock E2E |
| 固件 UI（`ui.rs`） | 可运行骨架（平铺菜单、确认、结果页）；完整页面进行中 |
| 固件事件循环（`app.rs`） | 可运行，完全由 mock 驱动 |
| RP2354 USB host 栈（cotton/TinyUSB） | **未开始** —— 先过硬件 G1 门禁（docs/plan/06 §7.2） |
| OLED / EC11 驱动、PCB | 未开始 |

## 构建与测试

```bash
# 主机测试（无需硬件：mock 模拟屏幕、编码器与手机）
cargo test --workspace

# Cortex-M33 固件构建
rustup target add thumbv8m.main-none-eabihf
cargo build -p auroku-core    --target thumbv8m.main-none-eabihf --release
cargo build -p auroku-firmware --target thumbv8m.main-none-eabihf --release
```

## 文档

- [06 可行性](docs/plan/06-rp2354-port-feasibility.md)
- [07 平台/工具链](docs/plan/07-rp2354-platform-and-toolchain.md)
- [08 核心库可移植性](docs/plan/08-core-portability.md)
- [09 OLED/按键交互](docs/plan/09-oled-buttons-ui.md)
- [10 no_std spike](docs/plan/10-core-nostd-spike.md)
- [13 会话规则](docs/plan/13-session-rules.md)

## 许可

MIT。原始 Python 实现来自 Yuu，详见 `LICENSE`。
