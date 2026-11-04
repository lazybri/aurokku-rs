# 14 · 本次会话上下文（rp2354 固件化：去桌面端 + mock 固件库 + rp2350-emu）

> 生成于 2026-11-04 18:30（Asia/Shanghai），作者：Lead（执行会话）。
> 基线：`rp2354` @ `7434d78`（工作规则入库）→ `dd02c41`（去桌面端 + 固件 workspace）→ **`3135689`**
> （本次会话全部 WIP + 队友 UI + emu 测试框架，18:22 由 lazybri 提交，已推送 origin/rp2354）。
> 用户指令（本次存档触发）：「把这次你和你队友的上下文存档起来」。
> 纪律：按 docs/plan/13 §1.4，上下文只由用户人工要求时留存；本文即本轮留存物。
> 关联：13 工作规则、06–10 RP2354 研究文档、11/12 前序上下文（docs/context）。

---

## 0. 一句话结论

本会话按用户重定义把 `rp2354` 从「桌面产品代码 + 研究文档」改造成**纯 RP2354 固件仓库**：桌面端代码/依赖/CI/文档全部移除；`crates/auroku-core`（协议核心，12 个操作）与 `firmware/auroku-firmware`（HAL/UI/协议/事件循环）落地；队友 `ui-oled` 完成完整 OLED 菜单 UI（29 个新测试）。**宿主侧 35 个测试全绿、fmt/clippy/thumbv8m 构建通过**；`rp2350-emu` 模拟器 mock 测试框架已建成并跑通 2/4 场景（scenario 0/3 PASS，scenario 1/2 因模拟器 Thumb-32 覆盖缺口失败）。**Kablix 阶段尚未开始**（按用户规则须等 rp2350-emu 全绿）。

---

## 1. 用户定义与补充（全文见 docs/plan/13）

1. 只能在 `rp2354` 分支内工作；允许本分支自动提交推送迭代；开始推理前先读先前上下文；会话上下文由用户最后人工要求留存。
2. **mock 测试路线**：先 `rp2350-emu`（失败项用 `rp2350-emu-tui` 检查）；**rp2350-emu 全绿后**才编写 Kablix mock 测试。
3. 本机在 **VPN** 环境：域名可能解析到非公网 IP（docs.rs 被网络策略拒绝）；依赖/源码优先走 Cargo + rsproxy。
4. 允许创建多个智能体队友（本会话创建了 `ui-oled`），队友受同一分支/推送/上下文规则约束。

---

## 2. 团队与任务板

| 成员 | 角色 | 任务 | 结果 |
|---|---|---|---|
| Lead | 架构/重构/模拟器/核心 | 去桌面端、核心 crate、固件骨架、rp2350-emu 框架、验收 | 进行中（emu 2/4） |
| `ui-oled` | 队友（fork 上下文） | task-1：按 docs/plan/09 实现完整 OLED UI + 宿主测试 | **completed**；33 个 firmware 测试通过 |

- 共享任务板：`task-1`（revision 3，owner `ui-oled`，status completed，写作用域
  `firmware/auroku-firmware/src/ui.rs` + `tests/ui_*`）。
- 队友报告摘要：重写 `ui.rs`（约 1200 行 diff），新增 `tests/ui_navigation.rs`（12）、
  `tests/ui_pages.rs`（12）、`tests/ui_app.rs`（5）；app.rs 契约未改；`cargo test -p auroku-firmware`
  33 passed、clippy/fmt/thumb 构建通过。
- 队友声明的与 docs/plan/09 偏差：UIM 读写清仍为顶层项（emu 场景固定“1 格到 Read UIM lock、3 格到危险项”）；
  Probe 未提供 layout/list；确认页不能显示旧锁值（`UiAction` 只有 `Submit(Task)`）；Type2Auth 自检/
  Device info/Settings 页面未实现。
- 说明：`3135689` 由 lazybri 在 18:22 提交，包含队友 UI 与 Lead 的 emu 框架；Lead 未另行提交该批文件。

---

## 3. 仓库最终结构与状态（HEAD `3135689`，与 origin/rp2354 同步）

```text
Cargo.toml                      # 虚拟 workspace：crates/auroku-core + firmware/auroku-firmware
                                #                 + firmware/emu-tests（host 测试）
crates/auroku-core/             # no_std + alloc 协议核心（与桌面共享；本分支已无桌面）
  src/{lib,error,codec,type2auth,model,transport,frame,session,report,ops,mock}.rs
firmware/auroku-firmware/       # RP2354 固件逻辑（no_std + alloc，host 可 mock）
  src/{lib,hal,app,protocol,mock,ui}.rs
  tests/{ui_app,ui_navigation,ui_pages,emu_parity}.rs
firmware/rp2354-emu-fw/         # RP2350 模拟器测试固件（独立 workspace；thumbv8m 专用）
  src/{main,allocator}.rs, link.x, build.rs
  rom/{bootrom-combined.bin,LICENSE-bootrom.txt,*.sha256}   # BSD-3 官方 bootrom（预留 TUI/未来）
firmware/emu-tests/             # 宿主 harness（rp2350-emu）：加载镜像、跑场景、读 mailbox
  src/lib.rs, examples/inspect.rs, tests/emulated_firmware.rs
scripts/emu-test.ps1            # 构建模拟器固件 + 运行 emu mock 测试
docs/plan/06–10,13,14           # RP2354 研究与上下文
spike/auroku-core-nostd/        # 早期 no_std 可行性 spike（保留证据）
```

- **已删除**：`src/` 桌面全部代码（lib/main/driver/auroku-gui）、`.github/scripts/` 桌面构建脚本、
  `AUDIT-REPORT.md`、`docs/windows-driver.md`；根 Cargo 桌面依赖/features 一并移除。
- **CI 重写**：`.github/workflows/build.yml` = 宿主 mock 测试 + rp2350-emu mock 测试 + thumbv8m 构建；
  桌面 release 流水线仅存在于 main/qt6。
- **README**：中英文改为固件分支说明（布局、状态、mock 测试命令、文档索引）。
- 关键提交：
  - `2be9b39` RP2354 研究文档 06–10 + spike（先前会话）；
  - `7434d78` docs/plan/13 用户工作规则；
  - `dd02c41` 去桌面端 + workspace/CI/README + core/firmware 骨架；
  - `3135689` 本会话全部后续 WIP：完整 UI、UI 测试、rp2350-emu 框架、bootrom 依赖、parity 测试、fmt。

---

## 4. 已交付内容

### 4.1 `crates/auroku-core`（协议核心，no_std + alloc）

- 逐行移植桌面 v0.3.2 协议层：codec、Type2Auth（MD5）、帧缓冲/超时/0xAF 过滤、base/UIMLK 会话与认证、
  **全部 12 个 Task**（status/readuimlk/writelk/clearlk/readiccid/readpin/resetpin/maintenance_bit/
  batch/probe seri|scdp/exituimlk/exit）；typed `Report` 枚举（替代 serde_json）。
- `UsbTransport` 同步 trait：`write/read/delay/now_ms`；`&mut T` blanket impl（固件复用同一 transport）。
- `mock::MockTransport`：**命令/响应脚本**（`on_write(expected, response)`），支持分片（`with_chunk`）、
  时钟推进、`violations` / `violation_writes`（便于定位协议顺序回归）。
- 语义变更（已记录）：`std::error::Error`→`core::error::Error`；`Task::Probe` 超时 `f64`→`u32 ms`；
  返回 JSON→typed report；probe list 属于宿主侧描述符操作，core 不实现。
- 验证：host `cargo check/test` 通过；`thumbv8m.main-none-eabihf` release 构建通过。
- **缺口**：core 缺少自己的单元测试（13 个迁移向量/E2E 尚未落到 `crates/auroku-core`；当前由
  `firmware/auroku-firmware/tests/emu_parity.rs` 间接覆盖 readuimlk 与 confirm 场景）。

### 4.2 `firmware/auroku-firmware`（固件逻辑，host 可 mock）

- `hal.rs`：`Display`（21×6 文本网格、flush）、`Input`/`InputEvent`（EC11 + BACK + 长按）、`Clock`。
- `protocol.rs`：`Session` 封装（`ProtoConfig`/`DeviceInfo`/`ProtoOutcome`），单任务同步执行。
- `app.rs`：`App<D,I,C,T>` 事件循环：input→UI→protocol→display；attach/detach、NO DEVICE、`link_mut` 诊断。
- `mock.rs`：`MockDisplay`（字符网格 + screen hash）、`MockInput`、`MockClock`。
- `ui.rs`（队友）：完整菜单树/滚动窗口/参数编辑（writelk hex+control、maintenance、batch、probe timeout）/
  长按确认（NOT UNDOABLE、BACK 取消）/全部 Report 结果页（mutation before+after、verified/partial、
  ICCID 分页、错误页四类、NO DEVICE、detach 复位）。
- 验证：`cargo test -p auroku-firmware` = **35 passed**（4 unit + 5 ui_app + 12 ui_navigation +
  12 ui_pages + 2 emu_parity）；fmt/clippy(-D warnings)/thumbv8m release 全部通过。

### 4.3 `rp2350-emu` mock 测试框架

- `firmware/rp2354-emu-fw`：RP2350 flash 镜像（vector table @0x10000000、picobin IMAGE_DEF @0x40、
  reset @0x60），Rust `global_asm!` 生成 picobin/vector；`no_main` + 24KB 静态 bump allocator
  （**非原子**，见 §5.2）；SRAM mailbox @0x20070000（scenario/status/check/detail/hash/magic/debug0-3）；
  4 个场景：0=alloc/Vec、1=App 全链路 readuimlk（脚本手机 + Type2Auth 向量）、2=危险操作确认页、
  3=无设备提交 NO DEVICE。场景完成后写 magic=`0x41555242`("AURB") 并 spin。
- `firmware/emu-tests`：加载 bootrom（合成 ROM：word0=SP 0x20080000、word1=firmware entry）+
  ELF 中 flash/RAM 段（`load_flash`/`load_image`），`poke` scenario，轮询 mailbox；
  `examples/inspect.rs` 打印 ELF 布局、入口字节、分块 PC/寄存器、fault frame（CFSR/HFSR/BFAR）、
  mailbox；`AUROKU_TRACE=1` + `AUROKU_EMU_QUANTUM_1=1` 可单步 trace。
- `scripts/emu-test.ps1`：构建 `rp2354-emu-fw`（thumbv8m release）→ 设置
  `AUROKU_EMU_FW_ELF`/`AUROKU_REQUIRE_EMU` → `cargo test -p auroku-emu-tests`。
- CI：新增 `emulator-mock-tests` job（构建 + 运行，env 强制要求 emu 测试不得静默跳过）。

---

## 5. 模拟器问题与已做的工作绕行（重要）

`rp2350-emu` = 0.2.6（crates.io 最新；picoem 上游同版本）；`rp2350-emu-tui` = 0.1.3。已克隆
picoem 仓库到 `%TEMP%\picoem`（含官方 bootrom、blinky/demo 固件与 TUI，未入库）。

### 5.1 真实 bootrom 路径不可用（上游已知）

- emulator 自身 flash-boot 测试 `test_flash_boot_blinky` 被 **`#[ignore]`**：bootrom
  `___step4_init_rcp_seeds` 的 CP7 canary/salt 与硅片不一致（`rcp_iequal` mismatch）。
- 实测：`load_bootrom(真实)+load_flash(我们的镜像)+reset()` 在 PC=0x3ee 处 NMI（IPSR=2）停机，
  mailbox 无输出。→ 因此 harness 改用**合成 bootrom**（word0=SP、word1=entry）直接进入 flash 镜像代码；
  固件仍是生产布局（XIP flash/vector/picobin），只替换 bootrom 阶段。
- 官方 bootrom 已入库 `firmware/rp2354-emu-fw/rom/`（BSD-3，附 LICENSE），等上游修复后可切回真实 bootrom
  并让 `rp2350-emu-tui` 直接使用。

### 5.2 已确认的 Thumb-32 覆盖缺口与绕行

| # | 现象 | 证据 | 绕行 |
|---|---|---|---|
| 1 | `lda`（Acquire load）被错误执行：`_start` 从 mailbox 读 scenario 后 PC 跳到 0（ROM 0 执行零字） | trace 单步：0x200072d6 `lda r10,[r0]` 后 PC=0；IPSR=3、BFAR=0x8000 | mailbox 原子操作全部 `Ordering::Relaxed`（单核无常中断） |
| 2 | `ldrex/strex` 独占监视器不可靠：`AtomicUsize` bump allocator 的 CAS 重试循环卡死 | trace：allocator `strex` 永远返回非 0 的分支循环 | allocator 改为**非原子** `static mut HEAP_NEXT` + `compiler_fence`（单核测试固件） |
| 3 | 其他布局敏感 fault（IPSR=3 / fault frame / `__aeabi_memcpy` 后 BFAR=0x8000） | 不同构建/优化级别下 1/2 场景行为变化 | 继续定位；当前 release（lto=true,codegen-units=1）对 0/3 稳定 |

### 5.3 当前 emu 场景状态（最近一次 `scripts/emu-test.ps1`，HEAD 后重跑）

| 场景 | 结果 | 备注 |
|---|---|---|
| 0 alloc/Vec | **PASS** | hash 0x0b916da4，500k cycles |
| 1 readuimlk 全链路 | **FAIL（超时）** | 200M cycles 内 mailbox 无结果（status=0/magic=0）；宿主 parity 同场景通过 |
| 2 确认页 | **FAIL（panic）** | status=3；panic message 非 str（detail=0x811c9dc5）；此前诊断到 line≈665、file hash 0x66fa540b、message len 17（疑似 “capacity overflow”）；宿主 parity 通过 |
| 3 NO DEVICE | **PASS** | hash 0xf8ece277 |

- 宿主同场景（`tests/emu_parity.rs`）均通过且 mock 零 violation → 1/2 的失败**不是** UI/协议逻辑错误，
  而是模拟器执行/指令问题或模拟器与生成代码的交互问题。
- 构建 profile 影响结果：曾试 opt-level=1/lto=false 更差；当前 release+lto+codegen-units=1 为最好基线。

---

## 6. 验证证据（本会话，HEAD `3135689` + 本次 fmt）

| 检查 | 命令 | 结果 |
|---|---|---|
| 宿主编译 | `cargo check --workspace --all-targets --offline` | PASS |
| 宿主测试 | `cargo test --workspace --exclude auroku-emu-tests` | **35 passed**（4+2+5+12+12） |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| fmt | `cargo fmt --all -- --check` | PASS |
| Cortex-M33 | `cargo build -p auroku-core/-p auroku-firmware --target thumbv8m.main-none-eabihf --release` | PASS |
| emu 固件构建 | `cargo build --manifest-path firmware/rp2354-emu-fw/Cargo.toml --target thumbv8m... --release` | PASS |
| emu mock 测试 | `scripts/emu-test.ps1` | **2/4 PASS**（0/3 过，1/2 见 §5.3） |

环境事实：rustc/cargo 1.99.1（2026）；已安装 `thumbv8m.main-none-eabihf` 与 `llvm-tools-preview`
（`llvm-objdump` 用于反汇编定位 fault）；cargo 源 rsproxy；VPN 环境（docs.rs 不可达，crates.io API 可用）。

---

## 7. 待办与下一步（推荐顺序）

1. **修 rp2350-emu 场景 1/2**（当前唯一阻塞项，也是 Kablix 阶段的前置条件）：
   - 用 `inspect` 的 `AUROKU_TRACE` + 单步 trace 找到 fault/hang 的精确指令；
   - 判定是模拟器漏实现还是生成代码触发的组合；可考虑：进一步降低触发面（避免某些 64 位/独占/字节后索引指令、
     固定 opt-level/codegen 组合）、给 `rp2350-emu` 打本地补丁并 vendor（需用户同意），或等待/升级上游；
   - 失败项按用户要求用 `rp2350-emu-tui` 交叉检查（TUI 需要 picoem 工作区 `roms/`；当前 bootrom 已在仓库，
     TUI 对自研 ELF 的加载方式需适配）。
2. **core crate 测试补齐（U1–U6 剩余）**：迁移桌面 13 个协议单测 + 12 个 Task 的 mock E2E 到
   `crates/auroku-core/tests/`，让 core 独立可回归（当前仅 firmware 侧 parity）。
3. **Kablix mock 测试**（仅在 1 全绿后开始）：先确认 Kablix（VS Code/Codium 离线 Pico/RP2040 模拟器）
   对 RP2354/自研固件的支持形态、可自动化接口（扩展/CLI/serial/UF2），再设计第二层 mock 测试。
4. **固件产品化**：异步/双核 proto_task + 阶段进度（docs/plan/06 §6.2、08 的 R7/R11）；
   cotton/TinyUSB USB host 栈（需硬件 G1 门禁，docs/plan/06 §7.2）；OLED/EC11 驱动与 PCB。
5. **未决问题（需用户决定）**：
   - 是否接受「合成 bootrom + flash 镜像」的 emu 测试形态（TUI/真实 bootrom 暂不可用）？
   - 是否允许 vendor/patch `rp2350-emu`（MIT/Apache-2.0）以补齐 Thumb-32/修复 RCP，从而让 1/2 与真实 bootrom 通过？
   - Kablix 的具体期望（自动化形式、固件输入格式、是否必须 RP2354 型号）？
   - 仓库内保留 32KB 官方 bootrom 二进制是否可以（BSD-3 已附许可）？

---

## 8. 复现命令

```powershell
cd C:\Users\Administrator\Documents\proj\aurokku-rs.worktrees\rp2354

# 宿主测试 / lint / 目标构建
cargo test --workspace --exclude auroku-emu-tests
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p auroku-core     --target thumbv8m.main-none-eabihf --release
cargo build -p auroku-firmware --target thumbv8m.main-none-eabihf --release

# rp2350-emu mock 测试（当前 2/4）
scripts/emu-test.ps1

# 单场景调试 / trace（fault frame、CFSR/BFAR、PC 轨迹）
$elf = 'firmware/rp2354-emu-fw/target/thumbv8m.main-none-eabihf/release/auroku-rp2354-emu-fw'
cargo run -p auroku-emu-tests --example inspect -- $elf 1
$env:AUROKU_TRACE='1'; $env:AUROKU_EMU_QUANTUM_1='1'
cargo run -p auroku-emu-tests --example inspect -- $elf 2
```

---

## 9. 本次会话的关键决策/偏差

1. 按用户重定义，`rp2354` 成为**纯固件分支**；桌面代码整体移除（main/qt6 仍保留桌面产品）。
2. core 传输 trait 选 **sync**（docs/plan/08 §6.3b）；固件 MVP 单任务同步执行，异步/双核留待后续。
3. `Task::Probe` 超时改 `u32 ms`（避免 M33 软浮点）；probe list 归宿主/固件描述符层。
4. 报告用 typed struct（core 不依赖 serde），JSON 契约由桌面/未来适配层重建；本分支已无桌面。
5. emu 测试使用**合成 bootrom + flash 镜像**（真实 bootrom 上游坏）；bootrom 入库仅作未来/TUI 预留。
6. emu 固件 allocator 非原子、mailbox 原子用 Relaxed（模拟器缺口绕行，非产品设计）。
7. 队友 UI 在 `ui.rs` 内实现完整设计，保持 app.rs 契约不变；emu 场景依赖的稳定字符串
   （CONFIRM/NOT UNDOABLE/NO DEVICE/readuimlk/锁值行）已在 13/UI 测试中固化。

---

## 10. 文件索引（本会话新增/修改）

| 路径 | 说明 |
|---|---|
| `docs/plan/13-session-rules.md` | 用户定义 + 补充（mock 顺序/VPN/多队友），本会话更新 |
| `docs/plan/14-session-context.md` | 本文 |
| `Cargo.toml`, `.cargo/config.toml`, `.gitignore` | 虚拟 workspace / 目标说明 / 忽略 |
| `.github/workflows/build.yml` | firmware CI：host + rp2350-emu + thumbv8m |
| `README.md`, `README.zh-CN.md` | 固件分支说明 |
| `crates/auroku-core/**` | 协议核心（12 Task + typed report + scripted mock） |
| `firmware/auroku-firmware/src/ui.rs` + `tests/ui_*` | 队友交付的完整 UI 与测试 |
| `firmware/auroku-firmware/src/{hal,app,protocol,mock}.rs`, `tests/emu_parity.rs` | Lead 交付的固件逻辑与 parity |
| `firmware/rp2354-emu-fw/**`, `firmware/emu-tests/**`, `scripts/emu-test.ps1` | rp2350-emu mock 测试框架 |
