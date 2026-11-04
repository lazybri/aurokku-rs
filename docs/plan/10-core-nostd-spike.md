# 10 · no_std 核心抽取 spike：lib.rs 纯协议层 → RP2354 可行性验证

> 生成于 2026-11-04（Asia/Shanghai）。**存储位置：`rp2354` 分支（已提交）**。
> 基线：main 工作树 `e594539`（auRokku v0.3.1），`src/lib.rs` 共 3139 行、19 个单测。
> 产出：本分支 `spike/auroku-core-nostd/` + 本报告；未改 src/、Cargo.toml、CI。
> 范围说明：本文只回答“核心协议库能否无 std 抽取并在 Cortex-M33 目标编译”。OLED/按键/外壳与整机方案不在本文范围（见同目录其它文档）。

---

## 0. 结论先行

【事实/已验证】**纯协议层可以零逻辑改动地抽成 `#![no_std]` + alloc 库，并在 RP2350 系列的真实编译目标上通过检查与构建。**

| # | 结论 | 证据等级 |
|---|---|---|
| 1 | 编解码 / SN 解析 / Type2Auth / 帧收发 / UIMLK 状态机可完全脱离 rusb、clap、serde_json、std::time | 已验证（编译 + 单测） |
| 2 | 13 个与协议相关的既有单测**逐字迁移后全部通过**（main 共 19 个测试，另 6 个是 host/CLI 专用，留在 std 侧） | 已验证 |
| 3 | 依赖只需 md-5 0.10.6（`default-features = false`）与 hex 0.4.3（`default-features = false, features = ["alloc"]`）；`cargo tree` 无任何 std feature/crate | 已验证 |
| 4 | `cargo check` 与 `cargo build --release` 在 `thumbv8m.main-none-eabihf`（Cortex-M33 hard-float）上均成功 | 已验证 |
| 5 | no_std 化的**语义变更只有 3 处**：可见性改 `pub`、`std::error::Error`→`core::error::Error`、`serde_json::Value`→强类型 struct | 已验证 |
| 6 | 真正的工作量不在 no_std 化，而在 ① 传输抽象与超时归属 ② 6 个操作的 JSON→强类型改写 ③ RP2354 侧 USB host 栈 | 推断（基于代码量 + 依赖分析） |
| 7 | 是否值得做：**值得做“core crate + host 回归”**；但 RP2354 能否稳定做 USB host 是最大未知，建议先 1–2 周硬件 bring-up 再决定是否全量移植 | 推断/待验证 |

**一句话**：协议算法本身“搬过去就能跑”（已验证），风险全在“怎么在 MCU 上当一个 USB 主机并和手机说话”（未验证）。

---

## 1. 交付物与文件清单

全部位于本分支 `spike/auroku-core-nostd/`：

| 文件 | 行数 | 内容 | 说明 |
|---|---|---|---|
| Cargo.toml | 18 | 独立 `[workspace]`，两个依赖关闭 std feature | 不被根 workspace 吞并；未改 main 的 Cargo.toml/Cargo.lock |
| src/lib.rs | 694 | 错误类型、codec、Type2Auth、helpers、`UsbTransport`、`FrameIo`、UIMLK 状态机、单测 | 每个函数注释标注 main 原始行号 |
| src/mock.rs | 66 | 内存版 `UsbTransport`（脚本化响应 + 可按 chunk 分片） | 让 E2E 单测无需硬件 |
| tests/uimlk_session.rs | 86 | “读 UIM lock”完整会话的 3 个集成测试 | 证明传输抽象与状态机可分离 |
| Cargo.lock | 自动生成 | 锁定 10 个包（含本 crate、运行时依赖与 1 个 build-dep） | 随本分支提交（`target/` 已忽略） |

`git status --short` 输出仅一行 `?? spike/`：main 的产品代码与依赖清单零改动。

---

## 2. 环境与基线（事实）

| 项 | 值 |
|---|---|
| main HEAD | `e594539`（分支 main） |
| rp2354 工作树 | `C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/rp2354`，分支 `rp2354` |
| rustc | 1.99.1 (58cf8f9c3 2026-10-13) |
| cargo | 1.99.1 (5f94df478 2026-08-27) |
| host triple | x86_64-pc-windows-gnu |
| 新增 MCU target | `thumbv8m.main-none-eabihf`（rustup 安装成功） |
| cargo 源 | rsproxy.cn（本机 config.toml 已配置）；全部命令加 `--offline` |
| 关键依赖缓存 | md-5 0.10.6、hex 0.4.3 等已在本地 registry（本机缓存可用） |

【事实】RP2350 系列为双核 Cortex-M33 @ 150 MHz、520 KB SRAM；RP2354 是内置 2 MB flash 的型号（来源：Raspberry Pi 产品页与 RP2350 数据手册，见文末链接）。因此 `thumbv8m.main-none-eabihf` 是正确的 Rust 目标；520 KB SRAM 也让 alloc 方案在容量上可行。

---

## 3. 验证命令与原始结果

### 3.1 host 单测（cargo test --offline）

~~~text
cargo test --offline
   Compiling auroku-core-nostd v0.0.0-spike
    Finished test profile in 1.47s
     Running unittests src/lib.rs
running 16 tests ... test result: ok. 16 passed; 0 failed; 0 ignored
     Running tests/uimlk_session.rs
running 3 tests ... test result: ok. 3 passed; 0 failed; 0 ignored
   Doc-tests auroku_core_nostd
running 0 tests
~~~

共 **19 个测试全绿**：
- 13 个从 main `src/lib.rs:2942-3059` 逐字迁移（nibble/subslice/hex_spaced/parse_sn/derive_key/auth_request_vectors/lockcode/endpoint）；
- 3 个新增帧解析单测（0x74 帧的合法/已清除/畸形输入）；
- 3 个 E2E 集成测试（见 §6）。

### 3.2 真实 MCU 目标交叉编译（已验证）

~~~text
rustup target add thumbv8m.main-none-eabihf
info: downloading component rust-std        # 成功，exit=0

cargo check --target thumbv8m.main-none-eabihf --offline
    Checking digest v0.10.7 ... Checking md-5 v0.10.6
    Checking auroku-core-nostd v0.0.0-spike
    Finished dev profile in 4.46s              # exit=0

cargo build --target thumbv8m.main-none-eabihf --release --offline
    Finished release profile in 3.53s          # exit=0；产出 rlib 189.5 KB
~~~

说明：库 crate 的 `cargo check/build` 不需要 panic handler 与全局分配器，因此这是“纯库级”验证；**真正链接成固件还需要 panic handler、allocator、启动代码与 USB 栈**（见 §9）。

### 3.3 依赖树审计（已验证）

~~~text
cargo tree --offline -e features
auroku-core-nostd
|-- md-5 v0.10.6
|   |-- cfg-if v1.0.5
|   +-- digest v0.10.7  (block-buffer, crypto-common, generic-array, typenum)
+-- hex v0.4.3  (feature "alloc")
~~~

【事实】树中没有任何 std feature 被启用（md-5 的 `default = ["std"]` 已被关闭；hex 只开 `alloc`）。md-5 的 docs.rs 分类本身即标注 no-std。

### 3.4 本轮未做/失败项（如实记录）

| 项 | 状态 | 原因 |
|---|---|---|
| 在真实 RP2354 上运行 | 未做 | 本机无硬件 |
| `cargo test --target thumbv8m` | 不适用 | 裸机目标无法运行 host 测试进程；host 单测已覆盖逻辑 |
| .text/flash 体积实测 | 未做 | 本机无 llvm-size / arm-none-eabi-size；rlib 189.5 KB 含元数据，**不能当 flash 占用** |
| allocator/堆峰值 | 未做 | 依赖固件全局分配器，非本 spike 范围 |
| 官方数据手册原文核对 | 部分受阻 | 本环境 web_fetch 被网络策略拒绝（非公开 IP），RP2350 USB host 细节只能依据搜索摘要，相关条目标【待验证】 |

---

## 4. 抽取范围与 LOC 估算（main 实测行号）

| 单元 | main 行号 | 原始行 | 非空行 | 纯代码行 | 迁移状态 |
|---|---|---|---|---|---|
| 错误类型 + Display + kind | 37-116 | 80 | 74 | 57 | 已迁移（去掉 rusb From） |
| 协议 codec + SN 解析 | 122-314 | 193 | 175 | 156 | 已迁移 |
| 传输层 send/recv/read_until | 655-758 | 104 | 93 | 92 | 重写为 FrameIo + trait |
| 会话/操作状态机 | 1164-1354 | 191 | 164 | 153 | 部分迁移（UIMLK 读） |
| 相关单测 | 2942-3059 | 118 | 103 | 101 | 13 个全部迁移 |

**汇总**：
- “纯协议核心”（错误 + codec + Type2Auth + helpers）≈ **273 原始行 / 213 纯代码行**；
- 加上传输层与会话状态机 ≈ **568 原始行 / 458 纯代码行**；
- 再加大约 100 行相关测试。即 **一个 500–700 行的 no_std crate 就能覆盖当前全部 SERI/UIMLK/Type2Auth 协议逻辑**，不包含 USB 枚举与 CLI/GUI。

注意：main 里这些全是**私有函数**；抽取时必须显式改 `pub` 并固定 API 边界，这也是唯一“非机械”的改动之一（无风险）。

---

## 5. 迁移中发现的障碍（逐条，带 main 行号）

| # | main 位置 | 障碍 | 本 spike 的处理 | 后续真实施工 |
|---|---|---|---|---|
| 1 | lib.rs:77 | `impl std::error::Error` | 改 `core::error::Error`（Rust 1.81 起稳定，项目 MSRV 1.88 满足） | 零成本，保留 |
| 2 | 679-713, 740-758, 1167-1194 | `Instant::now()` 截止时间 + `Duration` 超时 | 全部删除；超时下沉到 `UsbTransport` 实现 | 需定义“超时预算由谁给”：host libusb timeout / MCU 轮询次数 |
| 3 | 1319-1338, 1379-1439, 1494-1512, 1551-1556, 1652-1793 | 返回值是 `serde_json::Value`，内部大量 `json!` | 改为强类型 `UimLockState` / `UimLockReadOutcome` | 6 个操作各写一个 struct + std 侧 `From<T> for Value` 适配，保 CLI JSON 不变 |
| 4 | 21, 85-113, 331-367, 647-653, 765-867, 938-1047 | rusb 类型（`Context`/`DeviceHandle`/`ConfigDescriptor`）贯穿打开、枚举、`find_interface`、`inspect_usb` | 抽象出最小 `UsbTransport`（write/read），其余不迁 | 枚举/描述符探测需要新的“USB 描述符视图”抽象；可后置 |
| 5 | 1223, 1268 | `std::thread::sleep` 固定延时 | 删除（由 transport/时序层负责） | MCU 上换 cortex-m/embassy 定时器；延时值是协议时序的一部分，需保留常量 |
| 6 | 16, 374 | `LogSink = Arc<dyn Fn(&str) + Send + Sync>` | 核心层不承担日志；只保留 `async_events: Vec<u8>` | alloc::sync::Arc 可用，但 MCU 单线程下建议改成轻量 log trait |
| 7 | 695-703, 1186-1189 | 0xAF/0xE7 异步事件直接 `json!` | 核心只记录 opcode 字节 | 上层映射成原 JSON，保持输出兼容 |
| 8 | 680 | 固定 `[0u8; 512]` 接收暂存 | 降为 64（`ENDPOINT_CHUNK`） | 按端点包大小与最大帧 256 定；实测峰值 |
| 9 | 全文件 | Vec/String 遍布（SN、hex、错误消息、缓冲） | `extern crate alloc` | 需要 allocator；Rust 侧建议 embedded-alloc/自写 bump，预留堆 |
| 10 | 19, 2302-2370, 2717-2813, 2814 | clap `Parser/ValueEnum`、`Cli`、`cli_main` | 不迁 | 留在 host CLI crate |
| 11 | 2911, 2930, 3063, 3100, 3118, 3130 | 6 个 host/CLI 专用测试（sysfs、Windows 注册表、Task 命名、DeviceConfig 默认值） | 不迁 | 留 std 侧回归 |
| 12 | 1891-2177 | `SYSFS`/注册表/PowerShell 设备名补全 | 不迁 | 属 host 枚举层，与 MCU 无关 |

【结论】障碍集中在第 2/3/4 项（时间、JSON、USB 描述符），**算法与协议常量本身没有一处需要改**。这正面回答了用户“核心库能不能移植”的问题：能，且成本可估算。

---

## 6. UsbTransport 抽象与操作状态机分离（设计草案 + 证据）

### 6.1 trait 草案（已编译）

~~~rust
pub trait UsbTransport {
    fn write(&mut self, data: &[u8]) -> AuRokkuResult<()>;      // 必须整帧写完
    fn read(&mut self, buf: &mut [u8]) -> AuRokkuResult<usize>; // 至少 1 字节或 Timeout
}

pub struct FrameIo<T: UsbTransport> {
    transport: T,
    rx: Vec<u8>,              // 复刻 main rx_buffer
    awaiting_handshake: bool, // 复刻 0xAF 预回声过滤窗口
    async_events: Vec<u8>,    // 复刻 main async_events，但只存 opcode
}
// 方法：recv(expected_len) / read_until(terminator, limit) / send(frame)
~~~

设计要点：
- **超时归属**：main 的 `recv` 既做缓冲又做 `Instant` 计时（679-713）。核心层只缓冲；计时归实现方。这样 no_std 不需要任何时间 API，host 侧也不需要改协议逻辑。
- **握手过滤**：0xAF 剥除放在 `FrameIo::recv` 内（main 695-703 的等价物），保证“协议怪异行为”不在每个操作里重复。
- **操作状态机**：`read_uimlk()` = 握手 → 切 UIMLK → `0x54` → 解析 `0x74` → `0x56` 退出，只依赖 `FrameIo<impl UsbTransport>`。

### 6.2 E2E 证据（3 个集成测试，已通过）

| 测试 | 模拟内容 | 断言 |
|---|---|---|
| read_uimlk_end_to_end_with_fragmented_frames | 每次 read 只回 3 字节，0x74 帧被拆成 4 次 | 写序列严格为 1C 05 / E7 / 54 / 56；解析出 lock 值 12345678；退出被确认 |
| read_uimlk_reports_handshake_mismatch | 回显错误 1C 04 | 返回 Runtime 错误，且**不再**发后续命令 |
| read_uimlk_times_out_when_device_goes_silent | 只回握手，之后静默 | 返回 Timeout；写序列停在 E7 |

这组测试证明：**USB 后端与协议状态机可分离**，且真实端点“分片到达”的语义已被正确覆盖——这是后续在 MCU 上替换 transport 实现时最重要的回归资产。

---

## 7. 依赖与体积

| 依赖 | 版本 | 用途 | no_std 措施 | 备注 |
|---|---|---|---|---|
| md-5 | 0.10.6 | SERI 0x53 Type2Auth 的 MD5 | `default-features = false`（关掉 digest/std） | docs.rs 分类含 no-std |
| hex | 0.4.3 | SN/值 hex 编解码 | `default-features = false, features = ["alloc"]` | 只用 alloc 的 decode/encode |
| digest/block-buffer/crypto-common/generic-array/typenum/cfg-if | 传递依赖 | MD5 底层 | 由 md-5 feature 决定 | cargo tree 已确认无 std |
| version_check | 0.9.5 | generic-array 的 build-dep | 仅 build 期 | 不影响目标代码 |

体积：
- thumbv8m release 的 `libauroku_core_nostd.rlib` = **189.5 KB**。这是含 rustc 元数据的 rlib，**不是 flash 占用**。
- 【推断】协议代码 + MD5 的机器码预计 **5–15 KB** 量级（未经 size 工具实测；建议 bring-up 时用 `llvm-size` 或 `arm-none-eabi-size` 在最终固件里量）。

---

## 8. 工作量估算（面向决策）

| 阶段 | 内容 | 估算 | 风险 |
|---|---|---|---|
| A 已完成的 spike | 纯协议 core + 13 单测迁移 + 交叉编译 | 0.5–1 人日（本次已做） | 低 |
| B core 正式化 | 6 个操作的类型化 API、错误/日志抽象、全部操作状态机、更多单测 | 2–4 人日 | 低 |
| C host 回归 | std crate 实现 `UsbTransport`（rusb），把 typed 结果转回现有 JSON，跑 main 现有 CLI/GUI 测试 | 1–2 人日 | 低–中（JSON 兼容性靠现有测试兜底） |
| D RP2354 USB host bring-up | USB host 栈、枚举 au 手机、端点发现、时序/超时、稳定性 | **5–10 人日（最不确定）** | **高** |
| E 固件集成 | allocator、panic handler、OLED/按键、供电 | 另立任务（不在本文范围） | 中 |

**建议**：A+B+C 可先做（低风险，能立刻产出可被 CLI/GUI 与未来固件共用的 core crate）；D 做成一个独立 1–2 周 spike，用“能否枚举并完成一次 readuimlk”作为 go/no-go 门禁。

---

## 9. 下一步在真实 RP2354 上验证 USB host 的清单（建议按序）

| # | 验证项 | 通过标准 | 依据/备注 |
|---|---|---|---|
| 1 | 确认 RP2350 USB 控制器 host 能力与官方支持路径（原生 host / TinyUSB host / PIO 软实现） | 能跑通官方或社区 host 例程、枚举一个 U 盘/键鼠 | 【待验证】官方数据手册第 4 章 + pico-sdk TinyUSB host 例程；社区有 PIO host 项目 RP2350-USB-A |
| 2 | 5V VBUS 供电、限流、ESD 与手机枚举电流 | 手机能被稳定枚举、不触发过流 | 硬件设计项 |
| 3 | 枚举 au 手机（VID/PID 过滤） | 拿到设备/配置描述符，VID/PID 与 main scan_devices 一致 | 对齐 main lib.rs:765-867 的匹配逻辑 |
| 4 | 端点发现与类型 | 找到 EP OUT 0x04 / EP IN 0x84 及 bulk/interrupt 类型，等价 main find_interface（331-367） | 需在 MCU 侧实现描述符视图 |
| 5 | vendor interface 的 bulk 传输可用性 | 能发 1C 05 并收到回显 | main 不依赖 CDC 类驱动，按 vendor 端点操作 |
| 6 | 超时/重试语义 | 写 2s、读 3s 的默认预算可配置且行为与 host 一致 | 对应 main 663、679、1167 的硬编码 |
| 7 | 协议时序 | drain 50ms、非 0x0FCE 的 D8 预退出、E7 最多 4 次重试、0xAF 过滤均生效 | main 1220-1230、1171-1194 |
| 8 | 内存峰值 | rx 缓冲 + Vec + MD5 上下文峰值可接受，堆不碎片化 | 520 KB SRAM 有余量，仍需实测 |
| 9 | 稳定性 | 连续 100 次 readuimlk 无失败；拔线/超时可恢复 | 回归脚本 |
| 10 | 与 host 输出对照 | 同一台手机同一 SN，readuimlk/readpin/readiccid 的结果与 CLI 完全一致 | 用 main 的测试向量作 oracle |

---

## 10. 未决问题（需 Lead/用户决策）

1. RP2354 的 USB host 是否走原生控制器（需确认官方支持程度），还是必须 PIO 软实现？这决定 D 阶段的风险与工期。
2. core crate 的归属：新独立 crate（如 `auroku-core`）供 main 与固件共用，还是先在 spike 目录孵化后再迁入 main 的 workspace？
3. typed API 的 JSON 兼容策略：由 std 侧 `From<T> for Value` 生成，还是让 core 直接提供 serde 派生（会把 serde 依赖带进 no_std，需另评估）？
4. 固件是否真的需要全部 6 个操作（readuimlk/writelk/clearlk/readpin/resetpin/maintenance_bit），还是先做只读子集？
5. alloc 策略：全局 heap（embedded-alloc）还是尽量静态缓冲以避免碎片？
6. 固件工具链：embassy-rp vs pico-sdk + Rust HAL（rp235x-hal）；本 spike 只验证了 rustc 目标，未选 HAL。
7. 产品代码边界：main 是否接受“core 抽出后 src/lib.rs 改为 re-export”的重构（会触碰 src/，需用户批准）。

---

## 11. 复现命令（本机实测）

~~~powershell
# 1) host 单测
cd C:/Users/Administrator/Documents/proj/aurokku-rs/spike/auroku-core-nostd
cargo test --offline

# 2) MCU 目标安装与交叉检查
rustup target add thumbv8m.main-none-eabihf
cargo check --target thumbv8m.main-none-eabihf --offline
cargo build --target thumbv8m.main-none-eabihf --release --offline

# 3) 依赖审计
cargo tree --offline -e features

# 4) 确认未污染产品工作树
git -C C:/Users/Administrator/Documents/proj/aurokku-rs status --short
# 期望仅: ?? spike/
~~~

---

## 12. 结论

1. **技术上成立**：lib.rs 的纯协议部分（213 纯代码行）+ 传输/状态机（约 245 行）可组成一个 500–700 行、只依赖 md-5/hex 的 no_std 库；13 个既有测试零改动通过；Cortex-M33 目标 check/build 通过。
2. **成本可控**：正式化 + host 回归约 3–6 人日；相比“重写协议”，这是明显更优的路径。
3. **最大风险不在 Rust**：RP2354 的 USB host 可用性与稳定性（枚举、vendor bulk 传输、时序）是 go/no-go 关键；建议先做硬件 bring-up spike，再决定是否把固件做成产品。
4. **不要做的事**：不要把 clap/CLI/GUI/serde_json/rusb 带进固件；不要为了 no_std 重写协议算法；不要在没有硬件验证前承诺整机工期。

---

## 参考链接（外部结论来源）

- RP2350 产品页（双 Cortex-M33、520 KB SRAM、RP2354 带 2 MB flash）：https://www.raspberrypi.com/products/rp2350/
- RP2350 数据手册（USB 控制器/TinyUSB 参考实现相关章节）：https://pip.raspberrypi.com/documents/RP-008373-DS-rp2350-datasheet.pdf
- 社区 PIO USB host 实现（RP2350）：https://github.com/THLN33/RP2350-USB-A
- core::error::Error（Rust 标准库）：https://doc.rust-lang.org/stable/core/error/trait.Error.html
- Rust 1.81 发布说明（core::error::Error 稳定）：https://blog.rust-lang.org/2024/09/05/Rust-1.81.0.html
- md-5 0.10.6（feature 与 no-std 分类）：https://docs.rs/md-5/0.10.6/md5/
- hex 0.4.3（alloc/std feature）：https://docs.rs/hex/0.4.3/hex/
- TinyUSB（host/device 协议栈）：https://github.com/hathach/tinyusb
