# 06 · 把 main 的 auroku 核心库移植到 RP2354 + OLED/按键：可行性研究

> 生成于 2026-11-04（Asia/Shanghai），作者：Lead。
> 研究基线：main **e594539**（auRokku v0.3.1；src/lib.rs 与 v0.3.1 相同）；rp2354 分支基线 main **ee37a79**（v0.3.2，仅构建/驱动层变化）。
> 配套文档（同一 rp2354 分支，已提交）：
> [07 平台/USB host/自制板](07-rp2354-platform-and-toolchain.md)、[08 核心库可移植性](08-core-portability.md)、
> [09 OLED/按键交互](09-oled-buttons-ui.md)、[10 no_std spike](10-core-nostd-spike.md)。
> 证据分级：**【事实】**=代码/官方文档/实测可核对；**【推断】**=由事实推导；**【待验证】**=需真机或打样确认。
> 范围声明：本工具用于**自有或已获授权**的 au 设备；不讨论绕过他人设备或规避法律限制。本文只做技术可行性研究，不构成法律意见。

---

## 0. 结论先行

**结论：【可行，且比预期更顺】。** 这是一次"协议层小、平台能力够、工程量在固件与 UI"的移植，不是把桌面程序硬塞进 MCU。

1. **平台没有硬阻塞。** RP2354A/B = RP2350A/B + 封装内 2 MB QSPI flash，USB 控制器**原生支持 host**（FS 12 Mb/s + LS、支持 hub、硬件轮询 15 个 interrupt EP、4 KB DPSRAM）；TinyUSB 的 `hcd_rp2040.c` 是官方 reference implementation，不是 PIO-USB 偏方。【事实】
2. **核心库真正可移植的是"协议语义层"。** lib.rs 2574 代码行中约 **900 行（36%）** 是编解码/Type2Auth/操作序列；约 1390 行（54%）是 rusb/CLI/JSON/宿主枚举；driver.rs 719 行纯 Windows，MCU 全弃。【事实】
3. **协议层 no_std 化已被编译级验证。** spike 把 13 个协议单测逐字迁移，**19/19 通过**；`cargo build --release --target thumbv8m.main-none-eabihf` exit=0；依赖只剩 md-5(no_std)+hex(alloc)，无任何 std feature。【事实】
4. **协议不依赖 control transfer。** 全仓只有 bulk/interrupt 读写四种数据传输调用；枚举期控制传输（配置描述符、iSerialNumber、SET_CONFIGURATION）由固件 USB host 栈负责。传输抽象只需 4–6 个原语。【事实】
5. **Type2Auth 是纯 MD5 + 字节置换/位反转**，无 DES/AES、无随机数、无时间依赖；Type2Auth 材料来自 iSerialNumber（或手工 SN）。【事实】
6. **USB host 是最大工程风险，不是协议。** 风险按优先级：手机在非 PC host 上的枚举/充电策略 → host 栈成熟度（TinyUSB 的 RP2 HCD 有未关闭 bug；纯 Rust 的 cotton 的 RP2350 支持仍在 git main）→ 自制板 QFN/USB 差分/供电。
7. **推荐路线：纯 Rust 原生 host**——`rp235x-hal 0.4 + RTIC 2 + cotton-usb-host（git，CC0）+ auroku-core + ssd1306/embedded-graphics + EC11/BACK`。一条语言、无需 FFI，核心库直接复用；**C/TinyUSB 0.20.x 作为对照与兜底**（pico-sdk + Rust staticlib FFI + C++/u8g2）。PIO-USB 只在"必须同时当 USB device"时考虑。
8. **硬件建议：RP2354A（QFN-60）+ 4 层板 + USB-A host 口 + USB-C 5V 输入 + SWD/UART 调试口**；v1 不做双角色 USB，固件升级走 SWD（或预留 mux 位号）。整板首片 BOM 约 **$25–40**（不含外壳/电池/工具），5 片打样+SMT 约 **$120–200**。
9. **UI 可行但要求分级。** 1.3" 128×64 单色 I2C OLED（**必须锁 SSD1306/SSD1315**）+ EC11 编码器 + BACK；菜单覆盖 12 个 Task；写/清/重置 PIN 必须长按 1.5–2 s + 新旧值同屏 + NOT UNDOABLE；21 B raw hex 与 35 s 级 status 必须分页/分阶段进度。【事实+推断】
10. **建议先花 1 周做"最低成本证伪"，再投 PCB。** 用 Pico 2 + USB-A 转接板 + 5 V，验证：手机是否枚举、配置描述符是否读全、SERI 端点/alt 是否如桌面所见、能否完成一次 `readuimlk`。**这是 go/no-go 门禁**；通过后再画 v1 板。总工作量约 **30–55 人日**（兼职 2–3 个月，全职 4–6 周）。

---

## 1. 需求、范围与"核心库"的定义

### 1.1 用户诉求

把 main 分支现有的 auroku 核心库移植到 **RP2354**（RP2350 系列 MCU），用**一块 OLED 屏 + 若干按钮**交互操作核心功能；硬件为**完全客制化 PCB**、采购 RP2354 芯片贴装，不使用 Pico 2 成品板。

### 1.2 本文的"核心库"指什么

main 的 `src/lib.rs` 现在把五类东西放在一起（详见 08 §1）：

| 层 | 行区间 | 内容 | MCU 上怎么办 |
|---|---|---|---|
| L1 协议/编解码/鉴权 | 118–314 | nibble、SN 解析、Type2Auth/MD5、lockcode 解码 | **原样迁 core** |
| L2 USB 主机 I/O | 513–935、1144–1157 | rusb 句柄、claim interface、send/recv/drain/read_until | **换实现**（cotton/TinyUSB） |
| L3 任务/CLI/JSON 编排 | 1159–1815、2179–2900 | 会话状态机、12 个 Task、JSON、clap | 协议状态机迁 core；CLI/JSON 留桌面 |
| L4 枚举/扫描 | 316–367、938–1142、1856–2173 | 描述符枚举、probe、sysfs/注册表设备名 | 固件实现枚举/probe；宿主逻辑丢弃 |
| L5 宿主专属 | 79–116、1889–2090 + driver.rs | Windows 驱动、libusb 错误、注册表 | 全弃 |

**真正要"移植"的是 L1 + L3 的协议语义 + probe 的主动探测部分，约 900 行。** 这不是"重写整个 lib.rs"，而是**在 lib.rs 第 1159 行附近横切一刀**：把 `AuRokkuDevice` 的传输实现换成 `UsbTransport` trait，协议序列逐行照抄。

### 1.3 非目标（本阶段不做）

- Windows/Linux 驱动安装、GUI 的桌面功能（保留现有桌面程序不动）；
- 无线/联网、云端、手机端 App；
- 量产认证、外壳注塑、电池认证；
- 对非 au 设备/非授权设备的操作。

---

## 2. 事实基线（可核对）

### 2.1 核心库与协议【事实】

| 项 | 值 | 出处 |
|---|---|---|
| lib.rs 规模 | 3139 文件行 / 2574 代码行 | 08 §1 |
| 可移植协议内核 | ≈900 代码行（36%） | 08 §1.2 |
| 操作/报文 | 12 个 Task、15 条报文序列；单次收发 1–21 B | 08 §2 |
| USB 传输 | 只有 bulk/interrupt 读写；**0 处 control transfer** | lib.rs:650-651、665-666 |
| 端点 | 默认 EP OUT 0x04 / IN 0x84；Bulk/Interrupt 由描述符运行时判定 | lib.rs:29-31、316-328、331-367 |
| 超时 | 单次 recv 3 s（`DEFAULT_TIMEOUT`）；send 2 s；分片 500 ms | lib.rs:27、663、690 |
| 鉴权 | Type2Auth = MD5 + 置换/位反转；无 DES/AES/RNG/时间 | lib.rs:170–296 |
| 健壮性 | 非测试代码 0 个 unwrap、仅 2 处不可达 expect；索引前判长度 | 08 §7 R3 |
| 可移植比例 | 900 / 2574 ≈ 36%；L2+L3 外壳 54% | 08 §1 |

### 2.2 spike 编译级证据【事实】

代码在 `spike/auroku-core-nostd/`（本分支已提交；自带 workspace，未改 main 的 Cargo）：

| 验证 | 结果 |
|---|---|
| `cargo test --offline` | **19/19 通过**（16 单测 = 13 个 main 测试逐字迁移 + 3 个帧解析；3 个 E2E：分片到达/握手错误/静默超时） |
| `cargo build --release --target thumbv8m.main-none-eabihf` | **exit=0**（rlib 189.5 KB，含元数据） |
| `cargo tree` | 无 std feature；仅 md-5 0.10.6(default-features=false) + hex 0.4.3(alloc) |
| 规模 | 纯协议 213 行；含传输抽象+UIMLK 状态机约 458 行；500–700 行 crate 可覆盖全部 SERI/UIMLK/Type2Auth |
| 语义变更 | 仅 3 处：`pub`、`std::error::Error`→`core::error::Error`、`serde_json::Value`→强类型 struct |

> 这说明**"Rust 核心库上 MCU"不是理论推演，已经在本机编译并测试过**。剩余工作是把它正式化为 `crates/auroku-core`，并接入真实 USB host。

### 2.3 平台事实【事实】

| 项 | 值 | 出处 |
|---|---|---|
| RP2354A/B | RP2350A/B + 封装内 **Winbond W25Q16JVWI 2 MB QSPI flash**；引脚/封装完全兼容；不是新芯片 | RP2350 datasheet §14.3；[官方文档](https://www.raspberrypi.com/documentation/microcontrollers/rp2350.html) |
| 封装/GPIO | QFN-60 7×7 mm / 30 GPIO / 4 ADC（A）；QFN-80 10×10 / 48 GPIO / 8 ADC（B）；均 0.4 mm pitch | datasheet Table 1 |
| 内核/内存 | 双核 Cortex-M33 或 Hazard3 RISC-V @150 MHz；**520 KB SRAM**（10 bank）；3× PIO（12 SM）；2×UART/SPI/I2C；8 KB OTP | 官方文档/Product Brief |
| USB | **USB 1.1 控制器 + PHY，支持 host 和 device**；host 为 FS 12 Mb/s + LS 1.5 Mb/s、支持 hub 多级下游、硬件轮询 15 个 interrupt EP、4 KB DPSRAM | datasheet §12.7.1.1.2、§12.7.2.2、§12.7.3 |
| VBUS 控制 | VBUS_EN / VBUS_DET / OVCUR 三个**可选** GPIO；官方明说"不接也能用"（host 可常供电） | datasheet §12.7.3.10 |
| USB PHY | 内置上/下拉；host 模式自动弱下拉 DP/DM，**无需外部 15 k 下拉**；但 **DP/DM 必须 27 Ω 串阻** | datasheet §12.7.3.3；Hardware design §5.1 |
| 参考实现 | "The RP2350 TinyUSB port is the reference implementation"；host 即 `hcd_rp2040.c` | datasheet §12.7.4.1 |
| 限制 | 单 USB 控制器（host/device 软件二选一）、无 HS 480 Mb/s、无硬件 OTG 自动换角色；errata RP2350-E12：clk_sys 须比 48 MHz 快 ≥10%（默认 150 MHz 无碍） | 07 §2.2 |
| 启动 | RP2350 取消 RP2040 的 256 B boot2；bootrom 直接扫描 flash 镜像（前 4 KB 内需 ImageDef）；RP2354 内部 flash 按外部 flash 处理 | datasheet §5.2；rp235x-hal 示例 |
| 工具链 | `thumbv8m.main-none-eabihf` / `riscv32imac-unknown-none-elf` 可用；probe-rs 0.32 有 RP235x.yaml（走 bootrom 烧写）；rp235x-hal 0.4、embassy-rp 0.10、defmt 1.2 | 07 §3.3；本机实测 spike 已装 target |

### 2.4 USB host 软件栈现状【事实】

| 栈 | 状态 | 结论 |
|---|---|---|
| **TinyUSB 原生 host（C）** | TinyUSB 0.17.0（2024-09）加入 RP2350 host；pico-sdk 2.3.1 仍 pin 0.18.0；0.21.0 重构 RP2 HCD 并修 DPRAM unaligned memcpy | 可用，但需**覆盖 TinyUSB 版本**；0.21 有 #3875 回归（见下），建议 **0.20.x 或打补丁** |
| TinyUSB #3875 | RP2350B + hub，≥3 包 control IN（长配置描述符）只报第一包，status stage panic；0.20 正常、0.21 回归；workaround：host control 强制单缓冲 | 手机复合设备配置描述符可能 >128 B → **高优先级验证**；无 hub 且 0.20 可能规避 |
| TinyUSB #4310 | 挂设备时 `tuh_deinit()` 死锁 | 只影响去初始化；手持设备可规避 |
| **cotton-usb-host（Rust）** | crates.io 0.3.0 **只支持 RP2040**；RP2350 支持在 **git main（CHANGELOG Unreleased）**，有 `cross/rp2350-w6100-rtic2` 系统测试 | 纯 Rust 主线，但必须 **pin git commit 或 vendor**；单人维护（33★）、CC0；无 ISO、无 SET_INTERFACE、配置描述符 ≤256 B |
| embassy-usb-host 0.1.0 | crate 存在，但 embassy 主仓无该目录、无 RP HCD | 观望，不作为首选 |
| rp-pio-usb-host 0.1.0 | RP2040/RP235x PIO host，测试过 Pico 2；要 2 GPIO + 22 Ω/15 k + 1 个 PIO block | 备选，不作为 v1 主线 |
| Pico-PIO-USB（C） | RP2350 有源码分支/errata 规避，但文档仍写 RP2040；有丢包报告 | 只在需要"同时当 device"时考虑 |

> **平台结论**：RP2350/RP2354 原生 USB host 是官方能力；软件侧不是"有没有"，而是"选哪套、固定哪个版本、手机认不认"。

### 2.5 显示/输入/成本事实【事实/推断】

| 项 | 值 | 出处 |
|---|---|---|
| 屏 | 1.3" 128×64 单色 I2C OLED（SSD1306/SSD1315），¥12–18；0.96" ¥8–12 | 09 §1 |
| 帧率 | 400 kHz I2C 全屏 1024 B 理论 23 ms/帧（43 fps），实测 25–35 ms | 09 §1.2 |
| 输入 | EC11 带按压 + BACK（4 GPIO，约 ¥2–4）优于 3 键；4 键为降级 | 09 §4 |
| 中文 | 完整 GB2312 12×12 ≈122 KB / 16×16 ≈216 KB；固定文案子集 12px 仅 5–8 KB；MVP 建议英文/ASCII | 09 §2 |
| 字体许可 | u8g2/wqy 为 GPL 系，MIT 项目建议 OFL 的 Ark Pixel Font 子集 | 09 §2.3 |
| RP2354A 价格 | LCSC 参考 $1.2738(1+) / $1.2161(100+)，查询时库存 0；RP2354B $1.5335/$1.3829 | 07 §1.2 |
| 整板 BOM | 1 片 $25–40；10 片 $13–18；100 片 $7–11（不含外壳/电池/认证） | 07 §4.6 |

---

## 3. 可行性分项判断

| 维度 | 判断 | 依据 | 主要风险 |
|---|---|---|---|
| RP2354 能否做 USB host | **能（官方能力）** | datasheet §12.7.1/§12.7.4；TinyUSB/cotton 均有 RP2350 host 实现 | 软件版本/手机枚举，而非硬件 |
| 能否复用 Rust 核心库 | **能（编译级已验证）** | 10 号 spike：19/19、thumbv8m 构建通过 | 正式化 + JSON 兼容（08） |
| 能否用 OLED+按键操作全部功能 | **能，但需 UI 分级** | 09：菜单树覆盖 12 Task；128×64 需分页/确认 | 信息密度、误触、35 s 长操作 |
| USB 供电/VBUS | **可设计** | 5V 输入 + LDO + 限流开关；host 可常供电 | 手机充电策略、浪涌/反灌 |
| 烧录/调试 | **可设计（有取舍）** | 单 USB 控制器；SWD/UART + BOOTSEL；可选 mux | 每次升级需 SWD；UF2 需 device 口 |
| 自制板 | **可做，但不要手焊 QFN** | 官方 Minimal 参考 + 4 层 + SMT | QFN 0.4 mm、USB 差分、VREG 布局 |
| 工作量/成本 | **可接受** | ≈30–55 人日；首片 BOM $25–40 | 时间主要花在 host bring-up/UI |
| 先例 | **平台有，具体协议无** | MCU USB host 案例多；au SERI/Type2Auth 的 MCU 实现没有公开先例 | 必须真机验证，不能只看文档 |

**一句话**：这不是"能不能"的问题，而是"先证伪哪一件事、按什么顺序投入"的问题。

---

## 4. 推荐架构

### 4.1 总体分层

```text
┌──────────────────────────── 桌面（现有，保留） ────────────────────────────┐
│ CLI / egui GUI  ──  rusb 实现 UsbTransport  ──  serde_json 兼容层           │
│                         │                                                  │
│                  crates/auroku-core（no_std + alloc，共享）                │
│                   codec / type2auth / session / ops / report / transport   │
└───────────────────────────────┬────────────────────────────────────────────┘
                                │ 同一份协议状态机
┌───────────────────────────────▼────────────────────────────────────────────┐
│ RP2354 固件                                                                 │
│  ui_task（core0）：EC11/BACK → 菜单状态机 → ssd1306 128×64 I2C             │
│  proto_task（core1 或同核异步）：auroku-core → AuRokku vendor driver        │
│  USB host：cotton-usb-host（原生控制器）或 TinyUSB（兜底）                  │
│  存储：内部 2 MB flash（代码/字库/可选日志）；SRAM 520 KB                   │
└────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 推荐路线（B）：纯 Rust 原生 host

**技术栈**（按 07 的修正版）：

| 层 | 选型 | 理由 |
|---|---|---|
| MCU | **RP2354A**（QFN-60，30 GPIO，2 MB flash） | GPIO 预算 17–21，余 9–13；省外部 flash/6 条 QSPI 走线；比 B 便宜 $0.26 |
| HAL/PAC | **rp235x-hal 0.4 + rp235x-pac 0.2** | 与 cotton 的 RP2350 系统测试同一套；embedded-hal 1.0 I2C/GPIO/PWM |
| 执行器 | **RTIC 2**（+ rtic-monotonics） | cotton 的 RP2350 例程即 RTIC2；异步任务、中断、优先级成熟；也可换 embassy-executor（cotton future 与执行器无关） |
| USB host | **cotton-usb-host（git main，pin commit；CC0，vendor 到 `third_party/`）** | 原生 host，无需 PIO/额外电阻；control+bulk+interrupt+hub；async、no-alloc |
| 核心库 | **crates/auroku-core**（no_std+alloc，async trait） | 08 §6.3/§6.3b；与桌面共享；typed report |
| 显示 | **ssd1306 0.10（sync，embedded-hal 1.0）+ embedded-graphics 0.8.2** | 1 KB 帧缓冲；I2C 400 kHz 25–35 ms/帧；RTIC 任务内阻塞 flush 可接受（20–30 fps） |
| 输入 | EC11（A/B/SW）+ BACK，GPIO + RC 去抖 | 09 §4；比 3 键更适合长列表/hex |
| 调试 | **SWD（probe-rs 0.32）+ UART** + BOOTSEL/RUN 按键 | 单 USB 控制器被 host 占用；SWD 可救砖 |
| 启动/镜像 | RP2350 取消 RP2040 的 256 B boot2：bootrom 直接找 flash 镜像，要求前 4 KB 内有 **ImageDef**（`.start_block`）；rp235x-hal 示例已内置 `ImageDef::secure_exe()`；RP2354 内部 W25Q16JV 按外部 flash 处理 | 不需要自己写 boot2；烧录走 SWD/BOOTSEL/UF2 |

**为什么选它**：

- **一条语言**：核心库、USB 协议、UI 全是 Rust；无 C ABI/allocator/panic 跨语言问题。
- **原生 host**：不占 PIO、不需要 15 k 下拉/22 Ω，USB 差分只需 27 Ω；比 PIO host 少 2 个 GPIO 和一类已知丢包问题。
- **核心库直接复用**：cotton 提供 control + bulk/interrupt + 热插拔；我们的协议**不需要 class driver、不需要 ISO、不需要 SET_INTERFACE（alt=0 待确认）**。
- **桌面与固件共享同一份状态机**：proto_task 只提供 `UsbTransport` 的 cotton 实现。

**它的代价/风险**（必须写进决策）：

- cotton 的 RP2350 支持**尚未发版**（0.3.0 不含），要 git pin + vendor；单人维护（33★）。
- 无 SET_INTERFACE：若手机 SERI 接口 alt≠0，需要给 cotton 提 PR/打补丁（几十行）。
- RTIC2 与 09 文档的 embassy 栈不是同一套；09 的**菜单/交互/BOM 全部可复用**，只把 HAL/显示绑定改成 rp235x-hal + ssd1306 sync（09 已加兼容说明）。
- RP2354 内部 flash 由 bootrom 直接启动（无 boot2）；首板仍建议 SWD 救砖并验证 ImageDef/UF2 烧录。

### 4.3 兜底路线（A）：C/TinyUSB + Rust core FFI

| 项 | 内容 |
|---|---|
| USB host | pico-sdk 2.3.1 + **TinyUSB 0.20.x（`PICO_TINYUSB_PATH` 覆盖内置 0.18）**，用 host bare API：`tuh_edpt_open`/`tuh_edpt_xfer`/`tuh_descriptor_*_sync` |
| 核心库 | Rust `auroku-core` 编译为 no_std **staticlib + C ABI**（sync trait，见 08 §6.3b），C 侧提供 transport 回调 |
| UI | C++ + **u8g2**（中文点阵成熟），实现 09 的同一套菜单/文案/确认流程 |
| 调试/烧录 | 原生 USB 可留给 device/UF2？**注意**：host 模式占用同一控制器；TinyUSB host 下不能同时当 device。仍需 SWD/UART 或 mux |
| 优点 | 官方 host 参考实现，成熟；pico-examples 有 host 例程；无 cotton 风险 |
| 缺点 | 双语言/FFI/CMake+cargo；Rust staticlib 的 allocator/panic/错误映射；比 B 多 1–2 周；UI 换 C++ |
| 何时选 | Phase 0 证明 cotton 在手机上不可用，或团队更熟 C/C++，或需要 TinyUSB 的 hub/多设备能力 |

**TinyUSB 版本选择**：0.21 有 #3875（长 control IN 回归），优先 **0.20.x**；若必须 0.21，给 host control 强制单缓冲（issue 中 workaround）或在枚举期避免 >2 包 control IN。pico-sdk 的 0.18 太旧，需覆盖。

### 4.4 其他路线（不作为 v1 主线）

- **D. embassy-rp + rp-pio-usb-host**：保留 09 的 embassy 栈；PIO host 用 2 GPIO + 22 Ω/15 k + 1 个 PIO block，native USB 可留给 device/UF2。适合"必须插电脑就能烧/串口"的场景；但 PIO-USB 在 RP2350 有丢包报告，v1 不选。
- **E. Linux SBC（Zero 2 W 等）+ 现有 CLI + OLED/按键**：软件零移植、USB host 最稳；但体积/功耗/启动时间不适合手持。**若 RP2354 host bring-up 反复失败，这是最省事的兜底**。
- **F. STM32/ESP32-S3**：host 能力有，但 Rust 核心库要重写/FFI，收益低；不推荐。

### 4.5 架构决策门禁

**Phase 0 结束时按以下顺序拍板**：

1. 手机在 Pico 2 + USB-A 转接板上能否**枚举**、**读全配置描述符**、**找到 EP 0x04/0x84**、**读 iSerialNumber**？（E0）
2. cotton（git main）能否在同一手机完成 **bulk 往返**（至少 SERI echo 0x1C 0x05）？（E1）
3. 若 1 通过、2 不通过 → 转 A（TinyUSB 0.20.x + Rust core FFI）；若 1/2 都不通过 → 先排查手机/供电/线材，再考虑 E（Linux SBC）。
4. 若 2 通过 → 按 B 投板；**核心库正式化（08 的 U1–U6）与硬件打样并行**。

---

## 5. 硬件设计建议（自制板）

> 权威依据：[RP2350 datasheet](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf)、[Hardware design with RP2350](https://datasheets.raspberrypi.com/rp2350/hardware-design-with-rp2350.pdf)、官方 KiCAD Minimal 设计 [RP-010328-CA](https://pip.raspberrypi.com/documents/RP-010328-CA)（A）/ [RP-010329-CA](https://pip.raspberrypi.com/documents/RP-010329-CA)（B）。07 §4 有逐项 BOM/取值。

### 5.1 MCU 与最小系统

| 项 | 建议 | 备注 |
|---|---|---|
| MCU | **RP2354A** QFN-60 7×7 | 30 GPIO 够用；如需更多 IO/更好返修选 RP2354B（QFN-80 10×10，+$0.26） |
| 晶振 | **Abracon ABM8-272-T3 12 MHz** + 2×15 pF C0G + 1 kΩ 串阻 | USB 必须外部晶振；官方指定型号 |
| 内部 1.1 V VREG | **3.3 µH（Abracon AOTA-B201610S3R3-101-T，注意极性/方向）+ 3×4.7 µF + 33 Ω + 4.7 µF（VREG_AVDD RC）** | **布局照抄官方 Minimal**；官方原话：不照做 "at your own risk" |
| 3.3 V | LDO 或 buck，≥600 mA（NCP1117 1 A 最简；电池方案用 buck） | 手机 VBUS 另走限流开关，不与 3.3 V 混 |
| 去耦 | 每个电源脚 100 nF，靠近引脚；QFN-60 的 53/54 脚可共用一颗 | 官方 Minimal 取值 |
| Flash | **无外部 flash**（RP2354 内部 2 MB W25Q16JVWI） | QSPI 引脚仅用于 BOOTSEL/内部 die；省 6 条走线 + 1 颗器件 |
| BOOTSEL | QSPI_SS 经 **1 kΩ** 到按键到地；可选 10 kΩ 上拉 DNF | RP2354 同样需要；内部 flash 在 BOOTSEL 时高阻 |
| 复位 | RUN 按键到地 + 100 nF（可选） | 另留 SWD 复位 |
| SWD/UART | 6 针 1.27 mm：3V3/SWDIO/SWCLK/GND/UART_TX/UART_RX；另留 RUN/BOOTSEL 测试点 | v1 的烧录/救砖通道 |

### 5.2 USB host 口（最关键）

**v1 推荐 USB-A 母座做 host**：

- 手机用 A-to-C / A-to-micro 线即可；**无需 CC 电阻**，结构简单、便宜。
- D+/D- 各 **27 Ω 串阻，靠近芯片**；差分 **90 Ω**；下方完整地平面；4 层板优先（信号-地-电源-信号）。
- 座子侧加 **USBLC6-2SC6** 类 ESD（D+/D- 与 VBUS），先过 ESD 再进芯片。
- PHY 内置上下拉；**host 口不要放 1.5 kΩ 上拉**（那是 device 的 D+ 上拉），否则 host 检测异常。
- **VBUS**：5V 输入 → **限流负载开关（0.5–1 A，如 TPS2051/AP2553 类）** → USB-A VBUS；EN 接 GPIO（可选 VBUS_EN F10）。不要与输入 5V 直接短接；手机短路/浪涌会拖垮整板。
  - TinyUSB/cotton 的 RP2 HCD 默认**假设 VBUS 常供**（TinyUSB 直接强制 VBUS_DETECT override）；若要软件断电复位/省电，需自己控制负载开关 EN。
- **VBUS_DET/OVCUR 可不接**；若接，5 V 必须分压到 ≤3.3 V（GPIO 绝对最大 3.63 V）。
- **BC1.2 不做**：手机按 SDP（500 mA）识别，能枚举/能数据，够用；**切勿把 D+/D- 短接成 DCP**（只充电、不能数据）。若个别手机低电量不枚举，再考虑 CDP（TPS2543 类）。

**USB-C 做 host 的注意**：若坚持 USB-C，CC1/CC2 各 **56 kΩ ±5% Rp 到 VBUS/3.3 V**（Source 默认 USB 电流档），否则 C-to-C 线手机不认；还要处理 VBUS 放电/防反灌。v1 不推荐给自己加难度。

### 5.3 烧录/调试与单控制器取舍

RP2350 只有**一个 USB 控制器**，host 模式占用后不能同时当 USB device。三种做法：

| 方案 | 做法 | 评价 |
|---|---|---|
| **A（v1 推荐）** | 原生 host → USB-A；烧录/调试走 **SWD + UART**；BOOTSEL 键保留（救砖/工厂） | 最简单、host 最稳；升级需接调试器 |
| B | 原生 device（USB-C，UF2/CDC）+ **PIO-USB host**（GPIO12/13 → USB-A） | 开发体验最好，但 PIO 稳定性/15 k 下拉/禁 1.5 k 上拉等坑；作为 v1.1 备选 |
| C | 单 USB-C + **USB mux（TS3USB221 类）** 切换 host/device | 一个口两用；多一颗 mux + 控制 GPIO，SI 更复杂；可预留位号 |

**建议**：v1 走 A；PCB 上可预留 mux/第二 USB-C 的 0 Ω/未贴位号，v1.1 再评估。量产/测试至少留 SWD 4 针 + UART 2 针测试点。

### 5.4 显示、输入与面板

| 项 | 建议 | 备注 |
|---|---|---|
| OLED | **1.3" 128×64 I2C，SSD1306/SSD1315**（4 针排针或 1.0 mm FPC） | **BOM 锁定控制器**，避免 SH1106（页寻址/列偏移 2，ssd1306 初始化不适用） |
| I2C | I2C0 GPIO4/5；SDA/SCL 各 **4.7 kΩ** 上拉；模块自带则 DNP；100 nF + 4.7 µF 去耦；走线 <10 cm | 400 kHz；远离 USB 差分 |
| 输入 | **EC11（A/B/SW）+ BACK**，4 GPIO；RC（10 kΩ + 100 nF）+ 软件 20 ms 去抖；EC11 每相 100 nF | 比 3 键少误触；hex/长列表更快 |
| 提示 | 无源蜂鸣器（PWM 2.7 kHz）+ 2 个 LED（电源/状态） | 可选；蜂鸣器可静音 |
| 面板 | 屏窗 30.5×16.0 mm、1 mm 雾面盖板、EC11 右手位、BACK 左下；丝印 ROTATE/PUSH=OK/BACK | 09 §4.4 |
| ESD | 按键/编码器每线 1 kΩ + 100 nF；金属轴 1 MΩ∥4.7 nF 到 GND | 人体 ±8 kV 接触 |

### 5.5 电源树与 BOM

```text
USB-C 5V 输入（受电，CC 5.1k Rd）
  ├─ 3.3V LDO/Buck ≥600 mA ── RP2354 + OLED + 逻辑
  ├─ 限流负载开关（0.5–1 A）── USB-A VBUS（手机，SDP 500 mA）
  └─（可选）LiPo 充电 + 5V boost（TPS61023 类）── 电池版
```

| 分块 | 关键器件 | 1 片（$） | 备注 |
|---|---|---|---|
| MCU | RP2354A | 1.27 | LCSC 参考价；库存需确认 |
| 时钟 | ABM8-272-T3 + 2×15 pF + 1 kΩ | 0.45 | 官方指定 |
| VREG/去耦 | 3.3 µH + 4×4.7 µF + 33 Ω + ~40×0402 | 0.75 | 布局照抄 |
| 3.3V | LDO + 10 µF×2 | 0.25 | 或 buck |
| USB host | USB-A 座 + 限流开关 + USBLC6 + 27 Ω×2 | 0.60 | 关键 |
| 电源输入 | USB-C 座 + 5.1 k×2 + TVS/保险丝 | 0.25 | 受电 |
| 调试 | SWD 排针/JST + UART 测试点 + BOOTSEL/RUN 键 | 0.25 | 救砖 |
| 人机 | 1.3" OLED + EC11 + BACK + 蜂鸣器/LED | 2.3–3.0 | 见 09 BOM |
| PCB/SMT | 4 层 100×70 mm，5 片起 + 首单工程费 | $10–25/5 片 | **不要手焊 QFN** |
| **首片合计** | | **约 $25–40** | 不含外壳/电池/工具 |

> **贴装警告**：官方原话 QFN "very nearly impossible to solder without specialist equipment"；0.4 mm pitch + 散热焊盘必须钢网 + 回流焊。直接嘉立创/JLCPCB SMT，别手焊。

---

## 6. 交互设计（摘要，详见 09）

### 6.1 屏幕与菜单

- 1.3" 128×64 单色，**21×6 字符**（6×10 字体）；标题栏 + 内容区 + 底栏。
- 菜单树（与 `Task` 一一对应）：Status / UIM Lock（Read/Write/Clear）/ PIN·ICCID（Read PIN/Reset PIN/Read ICCID）/ Maintenance / Probe / Batch / Session（Exit/Exit UIMLK）/ Type2Auth 自检 / Device Info / Settings。
- 结果页：8 hex 锁值新旧同屏；21 B raw hex 每行 7 B 分页；ICCID 4 位分组；错误页按 `ValueError/RuntimeError/TimeoutError/DriverError` + UI 级 `NO_DEVICE/UNPLUGGED/POWER` 映射。
- 写/清 UIM lock、重置 PIN、维护位、退出 UIMLK：**长按 1.5–2.0 s 确认 + 新旧值同屏 + NOT UNDOABLE**；危险项默认焦点在 BACK；**任何单键路径都不能触发写**。
- SN 缺失时：设置页/首次连接提供 **16 hex 或 11 字符 SN 输入**（编码器逐位编辑）；否则 UIMLK 认证无法进行（08 R14）。

### 6.2 状态机与并发

- `ui_task`（core0）：1 ms 轮询编码器、20 ms 去抖按键、10 Hz 渲染；**不直接调协议**。
- `proto_task`（core1 或同核异步）：收 Command → `auroku-core`（cotton transport）→ Event 回报阶段/完成/错误；**不碰屏**。
- 单次传输默认 3 s，但整操作最坏：status ≈35 s、writelk/clearlk ≈26 s、readpin ≈9 s（08 §2.2）。固件**把单次传输超时降到 500 ms–1 s**、整操作预算 5–10 s，并在 UI 显示 `DRAIN → ENTER BASE → AUTH → READ/WRITE → VERIFY → EXIT` 阶段。
- `>45 s` 判 STUCK；**写命令一旦发出（0x72/0x57/0xDC 0x01）禁止取消**，显示 DO NOT UNPLUG；拔线后结果页红色 `partial_write_possible`，建议重连先 Status 读回。

### 6.3 语言/字体

- MVP：**英文/ASCII**（UIM/PIN/ICCID/SERI 本为缩写，hex/数字等宽，避免 0/O、1/l 误读）。
- V1.1 如需中文：只做固定文案 12×12 子集（5–8 KB），用 **OFL 的 Ark Pixel Font** 生成 1bpp 子集；不引完整 GB2312/wqy（GPL 许可）。
- 烧屏：对比度 0x8F、空闲 60 s 息屏、每 10 min 全屏偏移 ±1 px。

---

## 7. 实施路线、工作量与预算

### 7.1 阶段计划

| 阶段 | 内容 | 退出标准 | 人日（推断） |
|---|---|---|---|
| **P0 桌面 core 化**（可与 P1 并行） | 08 的 U1–U6：codec/type2auth/session/ops/report/transport trait + rusb 适配；桌面 JSON 字段契约保持 | 19+ 单测通过；桌面 `--op status --json` 与 v0.3.1 逐字段一致（golden） | 8–12 |
| **P1 host 证伪**（1 周，先做） | Pico 2 + USB-A 转接 + 5 V；E0 枚举/描述符、E1 cotton bulk echo、E2 readuimlk | 手机枚举 + 配置描述符读全 + EP 0x04/0x84 + SN + 一次 readuimlk | 3–6 |
| **P2 v1 硬件** | 原理图/PCB（4 层）+ 打样/SMT 5 片 + bring-up（电源/SWD/OLED/按键/USB） | 3.3 V/1.1 V 正常；SWD 可烧；OLED 0x3C 应答；EC11 每格 1 项；host 口枚举 U 盘 | 8–15 |
| **P3 固件 MVP（只读）** | ui_task + proto_task 骨架；Status/Read UIM/ICCID/Probe/Exit；错误/拔线恢复 | 真机连续 100 次读不失败；拔线可恢复 | 5–8 |
| **P4 写操作 + 稳定化** | Write/Clear/Reset PIN/Maintenance/Batch/Type2Auth 自检；长按确认；日志；烧屏/续航 | 回读校验与 report 一致；误操作演练 0 触发；72 h 固定画面无明显残影 | 5–8 |
| **P5 外壳/收尾** | 面板开孔/丝印、ESD、固件升级流程、文档 | 10 分钟乱按不触发写；BOOTSEL/SWD 可救砖 | 3–5 |
| **合计** | | | **约 32–54 人日**（含 core 与 host） |

> 平台估 26–47、core 估 11.5、UI 估 10–15；合并去重后取 **约 30–55 人日** 作为对外口径（表内细项 32–54）；兼职（每周 2–3 天）约 **2–3 个月**，全职约 **4–6 周**（不含 PCB 打样等待）。

### 7.2 最小验证实验（Phase 0，$15–30、1 周内）

1. **E0（0.5 天）**：Pico 2（仅作台架验证，不是最终硬件）+ USB-A 母座转接板 + 5 V 外部供电；跑 pico-examples `usb/host/host_cdc_msc_hid`（TinyUSB 0.20/0.21），先接 U 盘/键盘确认 host 通路；再接 au 手机，打印 device/config/string 描述符。
   - 看：是否枚举、配置描述符长度（是否 ≥3 包，触发 #3875）、SERI 接口号/alt/端点、iSerialNumber。
2. **E1（1–2 天）**：同一板跑 cotton 的 RP2350 例程（或最小裸机）→ 用手机做 **0x1C 0x05 echo**（SERI）与一次 0x54/0x56（若认证允许）。
3. **E2（1–2 天）**：把 no_std core spike 接到 E1 的 transport，跑通 **readuimlk**（最简完整会话）。
4. **门禁 G1**：E0–E2 全过 → 投 v1 板；只过 E0 → 转 A（TinyUSB 0.20 + Rust FFI）；都不过 → 检查线材/供电/手机模式，再考虑 Linux SBC。

### 7.3 成本

- 首片 BOM **$25–40**；5 片打样 + SMT 约 **$120–200**（含钢网/工程费，不含外壳/电池/调试器）。
- 调试工具：probe-rs 兼容调试器（¥30–100）、USB-A 转接板（¥5–10）、5 V 可调电源/电流表（已有可省）。
- 时间成本是主要成本：host bring-up 与 UI 占大头。

---

## 8. 风险登记（按优先级）

| # | 风险 | 概率/影响 | 触发点 | 缓解 |
|---|---|---|---|---|
| R1 | **cotton RP2350 支持未发版**（git main） | 中/高 | P1 E1 | vendor 到 `third_party`（CC0）、pin commit、写 200 行 HCD 回归测试；失败转 A |
| R2 | **TinyUSB #3875** 长 control IN（若走 A） | 中/高 | 枚举期读配置描述符 | 固定 0.20.x；或打补丁强制 host control 单缓冲；或枚举期避免 >2 包请求 |
| R3 | **手机在非 PC host 上不枚举/充电策略** | 中/高 | P1 E0 | 多种 A-to-C/micro 线；SDP；VBUS 负载开关可断电重插；必要时 CDP（TPS2543）；保留 Linux SBC 兜底 |
| R4 | **单 USB 控制器导致升级麻烦** | 高/中 | 每次固件更新 | v1 SWD + UART；BOOTSEL/RUN 键；预留 mux/第二口位号 |
| R5 | **QFN/USB 差分/贴装** | 中高/中 | P2 打样 | 4 层、官方 Minimal 布局、SMT、5 片、测试点；首片先焊 MCU/电源 |
| R6 | **VBUS 反灌/过流/浪涌** | 中/中 | P2 上电 | 限流开关 + TVS + 保险丝；输入与对外 VBUS 分离；5 V 不直连 GPIO |
| R7 | **core 正式化回归**（JSON 字段/错误文案） | 低/中 | P0 | 08 的 Report 字段=原 JSON 键；桌面 golden 对比；36 处 GUI 字段检查 |
| R8 | **长操作被误判卡死拔线** | 中/中 | P3/P4 写操作 | 阶段进度、45 s 看门狗、写命令后禁止取消、拔线后先读回 |
| R9 | **SN 缺失** | 低中/中 | P1/P3 | 枚举期读 iSerialNumber；失败给 16 hex/11 字符输入屏；否则 UIMLK 不可用 |
| R10 | **端点类型/alt 与桌面不同** | 低/中 | P1 | 桌面先 `--op probe --json` 确认 bulk_candidates/接口 alt；固件按描述符判定 bulk/interrupt，不写死 |
| R11 | **cotton 无 SET_INTERFACE** | 低/中 | P1 | 确认 alt=0；否则给 cotton 提 PR（几十行）或改 TinyUSB |
| R12 | **中文/字体许可** | 低/低 | V1.1 | MVP 英文；中文用 OFL Ark Pixel 子集，不用 wqy/u8g2 GPL 字库 |
| R13 | **RP2354A 缺货/假货** | 中/低 | 采购 | 只走 LCSC/授权代理；同板兼容 RP2350A + 外部 flash 的 BOM 变体；预留 U3 位号 |
| R14 | **ImageDef/内部 flash 启动与 UF2 兼容** | 低/中 | P2 首板 | 用 rp235x-hal 的 ImageDef 模板；Pico 2 先验证 UF2/SWD；首板留 SWD 救砖 |
| R15 | **法律/授权** | — | 全程 | 仅自有/授权设备；不对外销售（CE/FCC/CCC 另说）；不讨论绕过他人设备 |

---

## 9. 待决策清单（建议在 Phase 0 后拍板）

1. **固件主线**：B（cotton 纯 Rust）还是 A（TinyUSB C + Rust FFI）？→ 由 G1 门禁决定。
2. **core trait**：async（B/embassy）还是 sync（A/FFI）？08 §6.3b 已给两种草案；状态机共用。
3. **USB 口方案**：v1 USB-A + SWD（推荐）？是否预留 mux/第二 USB-C？
4. **MCU 变体**：RP2354A（推荐）还是 RP2354B（更多 IO/好返修）？是否保留 RP2350A+外置 flash 的 BOM 变体？
5. **屏幕**：1.3" 还是 0.96"？SSD1306/1315 具体型号？I2C 还是 SPI？
6. **输入**：EC11+BACK（推荐）还是 4 键降级？
7. **语言**：MVP 英文、V1.1 中文子集？还是首版就中文（+5–8 KB 字库/2–3 人日）？
8. **电池**：USB 供电（推荐 v1）还是 LiPo + 充电 + 5 V boost？
9. **MVP 范围**：只读 + Probe + Exit，还是含 Status（需 SN）？写操作放 P4。
10. **core crate 归属/桌面改造授权**：新建 `crates/auroku-core`、桌面 `run_task` 签名保持不变、JSON 由 Report 生成；需用户确认是否本轮就动 main 产品代码（**文档与 spike 已提交到 rp2354 分支，main 产品代码未改**）。

---

## 10. 结论

1. **平台没问题**：RP2354 的 USB host 是官方原生能力；FS 12 Mb/s + control/bulk 完全匹配 auRokku 的协议（1–21 B 短报文、3 s 级超时、无 ISO/无 HS）。
2. **核心库没问题**：协议层 ≈900 行、36%，spike 已编译+测试通过；无 std/allocator 障碍（alloc 可选）、无 panic/unwrap 陷阱；Type2Auth 是纯 MD5。
3. **真正的风险在"最后一公里"**：手机在自制 host 口上的枚举/充电行为、host 栈版本（cotton 未发版 / TinyUSB #3875）、以及 QFN/USB 打样。这三件事都可以用 **1 周、$15–30** 的 Pico 2 实验证伪，不需要先投 PCB。
4. **推荐路线**：B（rp235x-hal + RTIC2 + cotton + auroku-core + ssd1306）为主；A（TinyUSB 0.20 + Rust staticlib + u8g2）为兜底；PIO-USB/Linux SBC 为备选。**先验证再投板**。
5. **对用户问题的直接回答**：**能移植、能跑、能用手持 OLED+按键操作核心功能**；代价是约 30–55 人日 + 首片 $25–40 的硬件，最大不确定性是手机枚举与 host 栈，而不是 Rust 或协议。
6. **建议下一步（本周）**：买 Pico 2 + USB-A 转接板 + 1.3" SSD1306 模块 + EC11，按 07 §8.3 做 E0/E1；同时启动 08 的 core crate（U1–U6）。**在 G1 通过前不要画 PCB。**

---

## 附录 A · 文件与证据清单

| 文件/位置 | 内容 | 状态 |
|---|---|---|
| [07-rp2354-platform-and-toolchain.md](07-rp2354-platform-and-toolchain.md) | 平台/USB host/自制板/BOM/验证实验 | rp2354 分支，已提交 |
| [08-core-portability.md](08-core-portability.md) | 分层/时序/std 审计/trait/工作量/风险 | 同上 |
| [09-oled-buttons-ui.md](09-oled-buttons-ui.md) | 屏/字库/菜单/状态机/面板/BOM | 同上 |
| [10-core-nostd-spike.md](10-core-nostd-spike.md) | 19 测试、thumbv8m 构建、E2E 回放 | 同上 |
| `spike/auroku-core-nostd/` | spike 代码（694 行 lib + mock + E2E），独立 workspace | rp2354 分支已提交（`target/` 已忽略） |
| `src/lib.rs` / `src/driver.rs` | 被分析对象（未改动） | main 工作树 |

## 附录 B · 关键外部来源

- RP2350 数据手册（RP-008373-DS）：<https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf>（§12.7 USB、§14.3 Flash in package）
- RP2350 产品页/文档：<https://www.raspberrypi.com/products/rp2350/>、<https://www.raspberrypi.com/documentation/microcontrollers/rp2350.html>
- Hardware design with RP2350：<https://datasheets.raspberrypi.com/rp2350/hardware-design-with-rp2350.pdf>
- RP2350A/B Minimal KiCAD：<https://pip.raspberrypi.com/documents/RP-010328-CA> / <https://pip.raspberrypi.com/documents/RP-010329-CA>
- pico-sdk（2.3.1，tinyusb submodule 0.18.0）：<https://github.com/raspberrypi/pico-sdk>
- pico-examples USB host：<https://github.com/raspberrypi/pico-examples/tree/master/usb/host>
- TinyUSB RP2 HCD 源码：<https://github.com/hathach/tinyusb/blob/master/src/portable/raspberrypi/rp2040/hcd_rp2040.c>
- TinyUSB releases（0.17 RP2350、0.21 RP2 重构）：<https://github.com/hathach/tinyusb/releases>
- TinyUSB #3875（长 control IN 回归）：<https://github.com/hathach/tinyusb/issues/3875>
- TinyUSB #4310（deinit 死锁）：<https://github.com/hathach/tinyusb/issues/4310>
- cotton-usb-host（CC0；RP2350 支持在 git main）：<https://github.com/pdh11/cotton> / <https://crates.io/crates/cotton-usb-host>
- rp235x-hal 0.4：<https://crates.io/crates/rp235x-hal>；rp235x-pac 0.2：<https://crates.io/crates/rp235x-pac>
- embassy-rp 0.10：<https://crates.io/crates/embassy-rp>；embassy-usb-host 0.1：<https://crates.io/crates/embassy-usb-host>
- rp-pio-usb-host 0.1：<https://crates.io/crates/rp-pio-usb-host>；Pico-PIO-USB：<https://github.com/sekigon-gonnoc/Pico-PIO-USB>
- probe-rs RP235x target：<https://github.com/probe-rs/probe-rs/blob/master/probe-rs/targets/RP235x.yaml>
- SSD1306 datasheet：<https://cdn-shop.adafruit.com/datasheets/SSD1306.pdf>；ssd1306 crate：<https://crates.io/crates/ssd1306>；embedded-graphics：<https://crates.io/crates/embedded-graphics>
- Ark Pixel Font（OFL）：<https://github.com/TakWolf/ark-pixel-font>；U8g2 字体（GPL 注意）：<https://github.com/olikraus/u8g2>
- USB-C CC 电阻（TUSB320）：<https://www.ti.com/lit/ds/symlink/tusb320.pdf>；BC1.2（TPS2543）：<https://www.ti.com/lit/ds/symlink/tps2543.pdf>
- 价格/库存（2026-11-04 快照）：LCSC <https://www.lcsc.com/product-detail/C41378174.html>、<https://www.lcsc.com/product-detail/C39843328.html>；DigiKey SC1511-A4 <https://www.digikey.com/en/products/detail/raspberry-pi/SC1511-A4/28172162>

## 附录 C · 术语

- **SERI / UIMLK / Type2Auth**：au 手机的 USB 厂商协议/服务名；Type2Auth 为 0x52/0x53 挑战-应答（MD5）。
- **HCD**：Host Controller Driver；RP2 的 HCD 在 TinyUSB 中为 `hcd_rp2040.c`，cotton 中为 `host/rp2040.rs`。
- **SDP / CDP / DCP**：USB BC1.2 的三种下行口；SDP 可数据 500 mA，CDP 可数据+大电流，DCP 只充电（会毁掉数据）。
- **VBUS**：USB 5 V 供电；host 必须给 device 供 5 V，RP2354 本身不产生 VBUS。
- **RTIC 2 / embassy**：两种 Rust 嵌入式并发框架；前者用中断优先级+任务，后者用 async executor。
- **UF2 / BOOTSEL**：RP2350 bootrom 的 USB 大容量存储烧录模式；host 模式下需另找烧录通道。
