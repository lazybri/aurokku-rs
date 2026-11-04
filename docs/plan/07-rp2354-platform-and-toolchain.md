# 07 · RP2354 平台、USB Host 能力与自制板工程评估

> 生成于 2026-11-04 · 作者：teammate platform-researcher（task-1）· 位置：rp2354 分支 `docs/plan/`（已提交）
> 面向诉求：把 main v0.3.1 的核心库移植到 **RP2354（RP2350 系列）**，配 OLED + 按键做手持工具，且**用户自行设计客制化 PCB、自行采购 RP2354 芯片贴装**（不用 Pico 2 成品板）。
> 证据分级：【事实】= 官方/可复现来源，附 URL；【推断】= 基于事实的工程判断；【待验证】= 本次未实测、需打样/实验确认。
> 关联文档：08（核心库可移植性）解决"移植什么"，09（UI）解决"怎么交互"，本文只回答"平台能不能、板子怎么做、代价多大"。

---

## 0. 结论先行

**结论：【有条件可行】。** RP2350 系列的 USB 控制器**原生支持 host 模式**（不是只靠 Pico-PIO-USB 的社区偏方），软件栈有官方参考实现（pico-sdk + TinyUSB）与纯 Rust 选项（cotton-usb-host），硬件上 520 KB SRAM / 2 MB 内置 flash 对"协议 + 字库 + 日志"绰绰有余。真正的风险不在"能不能做 USB host"，而在三处工程细节：**(a) TinyUSB 的 RP2 HCD 在 RP2350 上仍有未关闭的 host bug；(b) 自制 QFN-60/80 板 + USB 差分走线对一次成功率的挑战；(c) 单 USB 控制器导致"host 口"和"USB 烧录口"互斥，必须在架构上二选一或加 mux。**

| 问题 | 结论 | 章节 |
|---|---|---|
| RP2354A/B 是什么 | RP2350A/B + 封装内堆叠 2 MB QSPI flash；其余引脚/外设完全相同 | §1 |
| USB 是否原生支持 host | **是**：FS host + LS 设备、支持 hub、硬件轮询 15 个 interrupt EP、4 KB DPSRAM | §2 |
| Pico-PIO-USB 是否必需 | **不**。原生 host 可用；PIO-USB 只在"需要同时当 USB device"或"要多一个 host 口"时才有价值 | §2.4 |
| 软件栈 | 主线建议 Rust：rp235x-hal 0.4 + RTIC 2 + cotton-usb-host（**git 依赖 pin commit，0.3.0 不含 RP2350**）；对照基线 C：pico-sdk + TinyUSB（覆盖到 0.20.x；0.21+ 需打 #3875 补丁） | §3 |
| Rust 工具链 | 可用：thumbv8m.main-none-eabihf、probe-rs 0.32（RP235x）、defmt；HAL 二选一：rp235x-hal 0.4（cotton 路线）或 embassy-rp 0.10（Embassy 路线） | §3.3 |
| 自制板难度 | 中高：0.4 mm pitch QFN + 0402 + 90 Ω 差分；官方原话"没有专业设备几乎焊不了 QFN" | §4.1、§4.5 |
| 内存/存储 | 够：2 MB flash、520 KB SRAM、USB 4 KB DPSRAM；UI framebuffer 仅 1 KB | §5 |
| 公开先例 | 有"MCU 做 USB host"的先例；**没有** au 手机 SERI/Type2Auth 在 MCU 上的公开实现 | §6 |
| 是否值得做 | 值得，但**分两步**：先用成品板验证 USB host + 协议，再做定制板 v1 | §8 |

**给决策者的三句话：**
1. 平台层面没有硬阻塞——USB host 是这颗芯片的官方能力，不是 hack。
2. 移植工作量的大头在核心库 no_std 化与 UI（见 08/09），不在 USB 驱动；USB 侧真有 bug 也只有 2 个已知、可规避。
3. 定制板请直接按 4 层板 + SMT 贴片（嘉立创/JLCPCB）做，不要手焊 QFN；v1 建议 **RP2354A（QFN-60）+ USB-A host 口 + SWD + UART**。

---

## 1. RP2354A/B 到底是什么【事实】

### 1.1 家族与差异

官方 Product Brief 的器件表（`rp2350-product-brief.pdf` p3）与数据手册 Table 1（`rp2350-datasheet.pdf` p14）完全一致：

| 型号 | 封装 | 内置 flash | GPIO | ADC 通道 | 备注 |
|---|---|---|---|---|---|
| RP2350A | QFN-60（7×7 mm） | 无 | 30 | 4 | 需外挂 QSPI flash |
| RP2350B | QFN-80（10×10 mm） | 无 | 48 | 8 | 需外挂 QSPI flash |
| **RP2354A** | QFN-60（7×7 mm） | **2 MB** | 30 | 4 | 与 RP2350A 引脚兼容 |
| **RP2354B** | QFN-80（10×10 mm） | **2 MB** | 48 | 8 | 与 RP2350B 引脚兼容 |

- **RP2354 = RP2350 + 封装内堆叠 flash**。数据手册 §14.3（p1328）明确：RP2354 内含两颗堆叠 die——同一颗 RP2350 die + 一颗 **Winbond W25Q16JVWI** QSPI NOR（2 MB）；"In all other respects, including pinout, they are identical to their flashless counterparts"。⇒ 现有 RP2350 的原理图/封装/固件可直接用，只是把外部 flash 去掉。来源：<https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf>
- **"内置"只是物理封装内，逻辑上仍按外部 flash 启动**：数据手册 p338 写 "In-package flash on RP2354 is external for boot purposes. It's a separate silicon die, and the RP2350 die doesn't implicitly trust it"。⇒ 启动流程（ImageDef 校验、UF2/PICOBOOT）与 RP2350 完全一样；安全启动签名仍针对这段 QSPI 内容。
- 其余关键规格（Product Brief p3）：双核 Cortex-M33（含 FPU/DSP）**或** 双核 Hazard3 RISC-V @150 MHz；**520 KB SRAM（10 个 bank）**；3× PIO、12 个状态机；USB 1.1 控制器 + PHY（host/device）；2×UART、2×SPI、2×I2C、24×PWM；8 KB OTP；生产承诺"至少到 2045 年 1 月"。
- 封装细节：两种 QFN 均为 **0.4 mm pitch**（60QFN e=0.400 BSC；80QFN e=0.400 BSC，数据手册 p6/p8），带散热焊盘（ePad）。

> 注意用词：官方称 "USB 1.1 controller and PHY"，但 PHY 跑的是 **USB 2.0 全速 12 Mb/s** 的电气/协议子集（数据手册 §12.7.1 直接写 "a Full Speed (FS) device (12 Mb/s)"）。不要因为 "1.1" 字样以为只能 1.5 Mb/s。

### 1.2 价格与供货（2026-11-04 实测查询）

| 型号 | 1+ | 10+ | 100+ | 1 000+ | 库存（查询时） | 整卷 |
|---|---|---|---|---|---|---|
| RP2354A | $1.2738 | $1.2491 | $1.2161 | $1.2062 | **0**（缺货） | 3400/卷 |
| RP2354B | $1.5335 | $1.4624 | $1.3829 | $1.3532 | **1** | 2500/卷 |

来源：LCSC 商品页 <https://www.lcsc.com/product-detail/C41378174.html>（RP2354A）、<https://www.lcsc.com/product-detail/C39843328.html>（RP2354B）。两页均标注"Reference price / Standard Packaging"，即参考价（未含运费/关税），不是原厂 1k 报价。

【推断】① 与 flashless 的 RP2350A（本次未查具体报价，量级 $0.7–0.9【待验证】）相比，RP2354 单颗贵约 $0.3–0.5，但省掉一颗 QSPI flash、4 条 QSPI 走线、一颗去耦和贴片工位，**净成本基本持平甚至更低**，且 7×7 mm 板上面积更小。② 查询时 RP2354A 显示 0 库存、RP2354B 仅 1 颗，属于"新料号 + 零售渠道"的常态，不代表长期缺货；但**打样前必须确认渠道**，并预留 RP2350A + 外置 flash 的 B 方案（同一 PCB 可兼容：留外部 flash 位号，RP2350A 贴上、RP2354A 不贴）。

---

## 2. 命门：RP2350/RP2354 的 USB 到底能不能当 host【事实】

### 2.1 官方依据（这条必须毫无疑问）

数据手册 §12.7.1（p1059）原文：

> "RP2350 contains a USB 2.0 controller that can operate as either: • a Full Speed (FS) device (12 Mb/s) • **a host that can communicate with both Low Speed (LS) (1.5 Mb/s) and Full Speed devices, including multiple downstream devices connected to a USB hub**"

Product Brief 的 Key features 也写 "1 × USB 1.1 controller and PHY, **with host and device support**"，引脚说明写 "USB controller, supporting full-speed device and **full-/low-speed host**"。

§12.7.1.1.2 Host Mode 能力（p1059）：

| 能力 | 规格 |
|---|---|
| 速率 | FS 12 Mb/s + LS 1.5 Mb/s（LS 设备可直连，也可经 FS hub，需 PRE token） |
| 拓扑 | 支持 hub，可挂多级下游设备（§12.7.2.2.2 还修了多级 hub 的 turnaround 超时） |
| 中断端点 | **硬件自动轮询最多 15 个 interrupt EP**（每 SOF 轮询一遍） |
| 缓冲区 | 4 KB DPSRAM（host 模式只需一套 EP 控制/缓冲寄存器 + 15 组 interrupt EP 寄存器） |
| 事务控制 | SEND_SETUP / SEND_TRANS / RECEIVE_TRANS / START_TRANS / STOP_TRANS / PREAMBLE_ENABLE / SOF_SYNC |
| NAK 处理 | RP2350 新增 `NAK_POLL.STOP_EPX_ON_NAK`：收到 NAK 可硬件停止事务（bulk 场景防丢数据） |
| VBUS 控制 | §12.7.3.10：VBUS_EN / VBUS_DET / OVCUR 三路可选 GPIO（"It is not necessary to connect up any of these pins"） |
| PHY | 内置上拉/下拉，**host 模式自动给 DP/DM 加弱下拉**，无需外部 15 kΩ |

RP2350 还修掉了 RP2040 的全部 host errata（RP2040-E3 interrupt EP buffer select、RP2040-E4 单缓冲状态写高半字），见 §12.7.2.1（p1060）。

### 2.2 但它做不到的事（设计约束）

| 限制 | 依据 | 影响 |
|---|---|---|
| **只有一个 USB 控制器**，host/device 由软件二选一 | §12.7.1、§12.7.3.2 "muxes a device controller or host controller onto a common set of components" | 跑 host 时不能同时当 USB 串口/UF2 设备；烧录/调试要另走 SWD 或复用（见 §4.4） |
| 无 High Speed（480 Mb/s） | §12.7.1.1 | au 手机在 SERI 模式下本来就是 FS 设备，**无影响**；但将来接 HS hub/存储盘不行 |
| 无硬件 OTG 角色自动切换 | 无 OTG/ID 自动检测寄存器，角色靠软件 + 外部 VBUS/CC 电路 | 做不了"插上就自动当 device"的双角色口，除非加 mux/CC 控制器 |
| `clk_sys` 必须比 `clk_usb`（48 MHz）快 ≥10% | errata **RP2350-E12**（§D.11，p1367） | 默认 150 MHz 没问题；低功耗降频到 ≤48 MHz 时 USB 会不可靠 |

### 2.3 TinyUSB 在 RP2350 上的现状【事实】+ 版本坑【事实】

- 数据手册 §12.7.4.1（p1070）明确：**"The RP2350 TinyUSB port is the reference implementation for this USB controller"**，文件为 pico-sdk 里的 `dcd_rp2040.c` / **`hcd_rp2040.c`** / `rp2040_usb.h`。⇒ 官方指定路径就是 TinyUSB，host 侧就是 `hcd_rp2040.c`。
- 当前 TinyUSB master 的 `hcd_rp2040.c` 里有 RP2350 专用分支：`#if defined(PICO_RP2350) && PICO_RP2350 == 1 → #define HAS_STOP_EPX_ON_NAK`（用上 §12.7.2.2.2 的新硬件特性）。来源：<https://github.com/hathach/tinyusb/blob/master/src/portable/raspberrypi/rp2040/hcd_rp2040.c>
- RP2350 支持是 **TinyUSB 0.17.0（2024-09-13）** 加入的：release notes 的 "RP2040" 小节写 "Add support for rp2350"。<https://github.com/hathach/tinyusb/releases/tag/0.17.0>
- **版本坑**：pico-sdk 2.3.1（最新 release，2026-09-04）的 `lib/tinyusb` submodule 仍 pin 在 **TinyUSB 0.18.0（2024-12-25）**（submodule sha 86ad6e5 = "Merge pull request #2913 from hathach/release-0.18.0"）。而 RP2 HCD 的重要修复在更晚版本：
  - 0.20.0（2025-11-20）；**0.21.0（2026-06-30）**：Refactor RP2 driver、Fix RP2350 hard fault in unaligned memcpy to USB DPRAM、RP2 增加 FreeRTOS 支持。
  ⇒ 用官方 pico-sdk 默认配置时，TinyUSB 落后 3 个 release；可用 pico-sdk 的 `PICO_TINYUSB_PATH` 指向自备的 TinyUSB 0.20.x（0.21+ 需打 #3875 补丁）。来源：<https://github.com/raspberrypi/pico-sdk>（`lib/tinyusb` submodule）、<https://github.com/hathach/tinyusb/releases/tag/0.21.0>

**RP2350 host 现存未关闭 issue（截至 2026-11-04，TinyUSB master）【事实】：**

| Issue | 状态 | 现象 | 对本项目影响 |
|---|---|---|---|
| [#3875](https://github.com/hathach/tinyusb/issues/3875) | open（2026-09-01） | RP2350B 原生 host（设备经板上 hub）：**≥3 个包的 control IN** 在双缓冲 EPX 上只报第一包 64 B 就"完成"，随后 status stage panic `buf_ctrl already available`；0.20.0 正常，0.21 回归 | **高**：枚举时读长配置描述符（手机多接口配置常见 >128 B）会踩到；缓解 = 用 0.20.x、或打补丁、或枚举阶段避免 hub/缩短描述符请求 |
| [#4310](https://github.com/hathach/tinyusb/issues/4310) | open（2026-10-25） | `tuh_deinit()` 在挂着设备时死锁（`hcd_deinit` 先释放 spinlock，`remove_device_tree` 再取同一把锁） | 中：只影响"热切换/去初始化"；手持设备一般不做 deinit，可规避 |
| [#3874](https://github.com/hathach/tinyusb/issues/3874) | closed（2026-09-01） | RX-timeout 路径遗留 EPX 已 arm，重试时 panic | 低：已修复，确认所用版本含修复即可 |

`hcd_rp2040.c` 源码里还有两处明确的"未实现"：`hcd_edpt_iso_activate` 未实现（**不支持等时端点**）、以及"已连接设备的重置（tuh_rhport_reset_bus）未做"的 TODO。本项目只用 control + bulk（+ 可选 interrupt），**不需要等时**，所以不影响。

### 2.4 Pico-PIO-USB 的真实定位【事实】

| 项 | 事实 |
|---|---|
| 官方 README 现状 | 仍写 "USB host/device implementation using PIO of raspberry pi pico (**RP2040**)"，功能表列 FS/LS host、hub、多端口、FS device；资源占用 1 PIO + 3 SM + 32 指令 + 2 GPIO + 15 KB ROM/RAM + 1 ms 定时器。来源：<https://github.com/sekigon-gonnoc/Pico-PIO-USB> |
| 源码对 RP2350 的支持 | 已存在：`src/pio_usb_ll.h` 有 `#if defined(PICO_RP2350) && PICO_RP2350`，含 **RP2350-E9 errata** 的 SRAM 规避（"Avoid rp2350_chip_version()/gpio_set_input_enabled() to make sure this is in SRAM"）；PR #204 "Use #if PICO_RP2350" 已合并 |
| 文档状态 | issue [#141](https://github.com/sekigon-gonnoc/Pico-PIO-USB/issues/141) "Document RP2350 support" **自 2024-09 起仍 open**；#181 也在问 RP2350A |
| 社区实测问题 | [#189](https://github.com/sekigon-gonnoc/Pico-PIO-USB/issues/189)（RP2350 host 偶发丢鼠标报告，1000 Hz→500 Hz）；[#203](https://github.com/sekigon-gonnoc/Pico-PIO-USB/issues/203)（RP2040/RP2350 + TinyUSB 下 MIDI host 偶发卡死） |
| 结论 | "能用、但文档未承诺、有已知稳定性问题"。**本项目不需要它**——除非要"host 口 + USB device 口同时存在"（见 §4.4 架构 B） |

### 2.5 auRokku 实际需要的 USB 子集（对照检查）

仓库 `src/lib.rs` 的 USB 用法（已读代码）：按 VID/PID 打开设备 → 遍历配置描述符，找同时含 **EP OUT 0x04 / EP IN 0x84** 的 interface → claim interface → 若 alt≠0 则 `set_alternate_setting` → 之后循环做 **bulk（或 interrupt）IN/OUT**，带超时与 drain；读 string descriptor 取 SN（Windows 路径另有注册表回退）。**没有** vendor 自定义 control 请求，**没有**等时端点。

| 需求 | RP2350 原生 host + TinyUSB | cotton-usb-host（Rust） |
|---|---|---|
| 枚举 + SET_CONFIGURATION | ✅ | ✅ |
| 读 device/config/string 描述符（control IN） | ✅（但注意 #3875 长 control IN） | ✅ |
| SET_INTERFACE（alt≠0） | ✅ | ❌ 源码未见 SET_INTERFACE（【待验证】：au 手机 SERI 接口大概率为 alt 0） |
| bulk IN/OUT + 超时 | ✅ | ✅ |
| interrupt IN/OUT | ✅ | ✅ |
| hub | ✅ | ✅（最多 15 hub / 31 设备） |

⇒ **协议层对 host 的要求很"素"**，两颗栈都覆盖，真正的差异在成熟度与语言（§3）。

---

## 3. 软件栈选项与工具链

### 3.1 四条路线对比

| 路线 | 成熟度 | 上手/维护成本 | 风险 | 适合 |
|---|---|---|---|---|
| **A. C/C++：pico-sdk 2.3 + TinyUSB host（0.20.x，用 PICO_TINYUSB_PATH 覆盖；0.21+ 需打 #3875 补丁）** | 官方参考实现；pico-examples 自带 `usb/host/host_cdc_msc_hid` 例程（<https://github.com/raspberrypi/pico-examples/tree/master/usb/host>） | 低（C 生态最全） | 中：#3875 未关；与 Rust 核心库需 FFI 或重写 | 快速验证协议、做"对照真相" |
| **B. Rust：rp235x-hal 0.4 + rp235x-pac 0.2 + RTIC 2 + cotton-usb-host（git pin）** | 纯 Rust、no_std、async、无 alloc；**RP2350 支持只在 git main**：CHANGELOG Unreleased 有「Add support for RP2350」、feature rp235x、src/host/rp2040.rs、cross/rp2350-w6100-rtic2 系统测试（main HEAD `6b43803e`，2026-09-08）；**crates.io 0.3.0 不含 RP2350 支持**（已下载 0.3.0 crate 核对：README/CHANGELOG 仅 RP2040、Cargo.toml 只有 rp2040 feature、源码无 rp235x 分支） | 中：需自己写设备类驱动（无 CDC/ACM 类，只有 HID/MSC）；须 `git = …, rev = 6b43803e` 或 vendor 进 third_party/ | **中高**：单人维护、下载量 3.4 k、git 依赖无版本约束；无 SET_INTERFACE | **推荐主线**（协议简单，正好不需要类驱动） |
| C. Rust + TinyUSB FFI（`tinyusb-sys`/手写 bindings） | 绑定层零散、无官方 Rust 绑定 | 高：C 构建系统（CMake）+ bindgen + 中断/临界区语义 | 高 | 不推荐 |
| D. Rust（替代/观望）：Embassy 系 —— embassy-usb-host 0.1.0，或 embassy-rp 0.10 + PIO-USB 绑定 | embassy-usb-host 0.1.0（2026-05 发布）API 有枚举/descriptor/control/class 框架；**但本次检查时 embassy main 分支未见 embassy-usb-host 目录**（仅 embassy-usb-driver/src/host.rs 有 HCD trait），且**没有 RP2350 的 HCD 实现**【待验证：是否随某分支/未合并 PR 发布】；embassy-rp 只有 device USB | 需自己写 HCD ≈ 重写 hcd_rp2040（或把 PIO-USB 移植成 Rust HCD） | 高 | 观望；也可「Embassy 管 UI/主循环 + RTIC 管 USB」分任务混跑，但要接受两套执行器 |

**推荐：B 为主线、A 为对照基线。** 理由：协议层简单（bulk + 枚举，无需类驱动），Rust 核心库直接复用（08 文档会给出 no_std 抽取方案），不存在 C 重写协议的成本；而 cotton 恰好提供了 RP2350 的 host/rp2040.rs（与 RP2040 同一外设）与 hub/热插拔支持。**B 的两个前提**：① 不能用 crates.io 0.3.0，必须 git pin（如 6b43803e）或 vendor；② 按 cotton 例程用 rp235x-hal + rp235x-pac + RTIC 2，不要指望 embassy-rp（它基于 rp-pac 7，是另一套 PAC）。风险用第一周 spike 消掉（§8.3）。

### 3.2 关键 crate 现状（crates.io，2026-11-04 查询）

| crate | 最新版 | 日期 | 说明 |
|---|---|---|---|
| `embassy-rp` | 0.10.0 | 2026-03-20 | features rp235xa/rp235xb，target thumbv8m.main-none-eabihf；但 **USB 只有 device（embassy-usb），无 host**；底层用 rp-pac 7（**不是** cotton 路线的 rp235x-pac 0.2） |
| `rp235x-hal` | 0.4.0 | 2026-02-01 | 同步 HAL（embedded-hal）；cotton 的 RP2350 例程用它 + rp235x-pac 0.2 + RTIC 2（thumbv8main-backend）+ rtic-monotonics 2.2.1 |
| `rp235x-pac` | 0.2.0 | 2026-01-31 | RP235x PAC；cotton 的 rp235x feature 依赖它（USB host 寄存器在其内） |
| `rp-pac` | 7.0.0 | 2025-01-06 | RP 系列 PAC（rp2040/rp235x feature）；**embassy-rp 用这一套**，与 cotton 的 rp235x-pac 0.2 不同，二者不能混用 |
| `cotton-usb-host` | 0.3.0（**不含 RP2350**） | 2026-06-02 | 0.3.0 只有 RP2040；RP2350 支持在 git main（HEAD `6b43803e`，2026-09-08，未发版）⇒ 必须 git pin/vendor；限制：最多 31 设备/15 hub、配置描述符 ≤256 B、**无等时**、仅 LS/FS |
| `embassy-usb-host` | 0.1.0 | 2026-05-03 | 实验性；无 RP HCD |
| `embassy-usb` | 0.6.0 | 2026-03-20 | 仅 device |
| `probe-rs` | 0.32.0 | 2026-07-22 | flash/RTT/defmt |

### 3.3 Rust 工具链可用性（本机实测【事实】）

- `rustup target list` 中 **`thumbv8m.main-none-eabihf`**（Cortex-M33F，RP2350 Arm 核）与 **`riscv32imac-unknown-none-elf`**（Hazard3）均可用；本机 rustc 1.99.1。**本机尚未安装**这两个 target（`rustup target add` 即可，需联网）。
- probe-rs 有 `probe-rs/targets/RP235x.yaml`：Arm 双核（armv8m，APv2 0x2000/0x4000）+ RISC-V 双核，flash 算法走 bootrom 流程（probe-rs changelog："Changed the RP2350 to run through the bootrom sequence to increase programming speed"）。⇒ `probe-rs run`/`cargo embed` + defmt-rtt 可用。<https://github.com/probe-rs/probe-rs/blob/master/probe-rs/targets/RP235x.yaml>
- defmt 1.2.0、panic-probe 生态正常；embassy 的 `time-driver`/`executor-thread` 与 RP2350 匹配。
- **cotton 的 RP2350 例程组合**（cross/rp2350-w6100-rtic2/Cargo.toml 实测）：rp235x-hal 0.4（critical-section-impl/rt/defmt）+ rp235x-pac 0.2 + rtic 2.1（thumbv8main-backend）+ rtic-monotonics 2.2.1（rp235x）+ cotton-usb-host（git，feature rp235x）+ cotton-usb-host-msc + cotton-scsi + static_cell 2。**这套与 embassy-rp 0.10（rp-pac 7 + embassy-usb device）不是同一套**，不要混用；选 B 就按这套搭工程。

### 3.4 "如果只能用 C/C++"意味着什么

| 方案 | 代价 |
|---|---|
| 用 C 重写核心库协议层 | 08 文档会说明 lib.rs 里协议/编解码/Type2Auth 是纯逻辑；重写 = 放弃现有单测与 Rust 类型安全，工作量按"重写 3–4 k 行"计，**不建议** |
| Rust 核心编译成 staticlib 给 C 调（FFI） | no_std staticlib + `cbindgen` + C 侧 CMake 链接可行，但要处理 allocator、panic handler、跨语言错误映射；比"Rust 全套"多 1–2 周且长期双语言维护 |

**结论：没有必要走 C**。Rust 侧唯一缺的是"成熟的 RP2350 USB host 栈"，而 cotton 已经补上了 80%；反之走 C 则要重写整个核心库。真正需要 C 的地方只是"用 pico-sdk 例程做对照实验"。

---

## 4. 自制板硬件设计（客制 PCB 视角）

> 本节面向"自己画板、自己买 RP2354 芯片贴装"。官方参考设计必读：**《Hardware design with RP2350》**（RP-008279-DS，2026-08 版）<https://datasheets.raspberrypi.com/rp2350/hardware-design-with-rp2350.pdf>，配套 KiCAD 工程 **RP2350A Minimal = RP-010328-CA**、**RP2350B Minimal = RP-010329-CA**（解压后为完整原理图/PCB/BOM）：
> - <https://pip.raspberrypi.com/documents/RP-010328-CA>（zip，约 900 KB）
> - <https://pip.raspberrypi.com/documents/RP-010329-CA>（zip，约 1.05 MB）

### 4.1 RP2354A（QFN-60）vs RP2354B（QFN-80）怎么选

先算 GPIO 预算（本项目）：

| 用途 | 数量 | 备注 |
|---|---|---|
| USB host D+/D- | 2 | 固定引脚 USB_DP/USB_DM（51/52），不占 GPIO |
| OLED I2C（SDA/SCL） | 2 | 任意 GPIO，可共享 |
| 按键 | 4–5 | 矩阵或独立，带上拉 |
| 蜂鸣器 | 1 | PWM |
| 调试 UART | 2 | 建议留测试点 |
| SWD | 2 | SWDIO/SWCLK 固定引脚 |
| VBUS_EN / VBUS_DET / OVCUR | 0–3 | 见 §4.3，可只留 VBUS_EN + 软件过流 |
| 电源指示/用户 LED | 1–2 | |
| 预留（second QSPI CS / 传感器 / 扩展） | 2–4 | GPIO0 可作 XIP_CS1n |
| **合计** | **约 17–21** | |

| 维度 | RP2354A（QFN-60, 7×7） | RP2354B（QFN-80, 10×10） |
|---|---|---|
| GPIO / ADC | 30 / 4 | 48 / 8 |
| 本项目是否够 | ✅ 够，余 9–13 个 | ✅ 富余，可加 SD 卡/扩展排针 |
| 焊接难度 | 0.4 mm pitch，60 pin，7×7 mm 更密 | 0.4 mm pitch，80 pin，但 10×10 mm 引脚间距物理上更宽松，**返修更容易** |
| 走线 | 30 GPIO 已够，但 QFN-60 出线拥挤（官方 Minimal 也要 0402 + 双层紧排） | 出线宽松，适合 4 层板 + 更多外设 |
| 单价（1+ / 100+） | $1.2738 / $1.2161 | $1.5335 / $1.3829 |
| 供货 | 查询时 0 库存 | 查询时 1 颗 |
| 适配参考设计 | RP-010328-CA | RP-010329-CA |
| 结论 | **v1 首选**（够用、便宜、板小） | 需要更多 IO / 更好焊 / 想留扩展时选 |

【推断】两者贴装难度差别不大（都是 0.4 mm QFN + 散热焊盘），差别主要在**出线空间**和**返修手感**；如果这是你第一次贴 QFN，QFN-80 的 10×10 mm 会明显宽容一些。价格差 $0.26（1+），可忽略。

### 4.2 最小系统（照抄官方 Minimal 设计即可）

官方 Minimal 设计（RP-010328/29-CA）的 BOM 与取值，均已从《Hardware design with RP2350》核对：

| 功能块 | 官方做法（RP2350 Minimal） | 用于 RP2354 时 |
|---|---|---|
| 12 MHz 晶振 | **Abracon ABM8-272-T3**，30 ppm、ESR ≤50 Ω、CL=10 pF；两侧 **15 pF C0G** 到地；串联 **1 kΩ**（R2）防过驱 | 不变（USB 必须用外部晶振；内部 RC 精度不够） |
| 内部 1.1 V 稳压器 | VREG_VIN=3.3 V；**L1 = 3.3 µH Abracon AOTA-B201610S3R3-101-T**（有极性点、编带同向）；C6/C7/C9 = **4.7 µF 0402**；VREG_AVDD 经 **33 Ω + 4.7 µF** RC 滤波；VREG_PGND 单独回流到 PGND 引脚 | 不变；**布局必须照抄**（官方原话：不照做"at your own risk"） |
| 3.3 V 输入 | NCP1117（1 A LDO）+ 10 µF 输入/输出 | 可换更高效的 buck（若用电池/发热敏感） |
| 去耦 | **每个电源引脚 100 nF**，尽量靠近；QFN-60 上 53/54 脚共用一颗（空间妥协） | 不变 |
| flash | 外部 W25Q128（16 MB）U3 | **RP2354 直接删掉 U3**；保留 BOOTSEL 到 QSPI_SS 的 1 kΩ + 按键 |
| BOOTSEL | QSPI_SS 经 **1 kΩ（R6）** 到按键到地；可选 10 kΩ 上拉（R1，默认 DNF） | 不变（RP2354 也必须保留） |
| 复位 | RUN 引脚按键到地 | 不变 |
| SWD | 3-pin JST（SWDIO/SWCLK/GND）或排针 | 建议保留（见 §4.4） |
| 第二 flash / PSRAM | RP2350B Minimal 预留 U4，CS 接 **GPIO0（XIP_CS1n）**，R11/R13 选择 | RP2354 可用同一路做「第二 flash 或 PSRAM」；v1 不建议上，留 footprint 即可 |

启动与烧录（【事实】+【推断】）：
- RP2354 的 2 MB flash 在 QSPI_SS 上，**bootrom 把它当外部 flash 探测**（0x03 单线读、CPOL/CPHA 四组合试 CRC），所以启动行为与 RP2350 一致；UF2 拖放、PICOBOOT、`probe-rs` 都照旧。
- 官方提示：UF2 + 分区表在早期芯片 rev 上有 errata（RP2350-E10，仅影响 A2，A3 已修）；自用可忽略。
- **启动/镜像【事实】**：RP2350 **取消了 RP2040 那个 256 B 的 boot2**——bootrom 直接从 flash 扫描镜像，只要求**前 4 KB 内**出现合法 ImageDef（datasheet §5.2：「removal of a boot2 in the first 256 bytes of the image, and the new requirement for a valid image definition anywhere within the first 4 kB」；另见 "Unlike RP2040, there is no requirement for flash binaries to have a checksummed boot2 flash setup function at flash address 0"）。Rust 侧由 `rp235x-hal` 的 `ImageDef::secure_exe()` 放进 `.start_block` 段（rp235x-hal-examples 的 `memory.x`/`blinky.rs` 无 `.boot2` 段；`embassy-rp` 的 `boot2-*` feature 是 RP2040 专用，RP235x 不用）。RP2354 封装内 W25Q16JV 仍按外部 flash 处理。首板留 SWD 救砖，并分别验证 UF2 与 SWD 两条烧录路径。
- 【推断】省掉外部 flash 后，QSPI 只剩芯片到封装内 die 的键合，**PCB 上不需要任何 QSPI 走线**——这是 RP2354 相对 RP2350 最大的板级收益（少 6 条高速线 + 少一颗器件 + 少一个贴片位），对 2 层/4 层自制板都友好。

---

### 4.3 USB host 口设计（本项目最关键的一段）

#### 4.3.1 用 USB-A 还是 USB-C 做 host 口

| 方案 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **USB-A 母座（host）** | 手机侧用 A-to-C 线即可；**无需 CC 电阻**；VBUS 直给；结构简单、便宜 | 座子大；不能 C-to-C | ✅ **v1 首选** |
| USB-C 母座做 host（Source/DFP） | 可用 C-to-C 线；体积小 | **必须给 CC1/CC2 各加 Rp 上拉（默认 USB 电源档 = 56 kΩ ±5% 到 VBUS/3.3 V）**，否则手机（sink）不会认这是一个 source，C-to-C 线会"没反应"；还要处理 VBUS 放电/防反灌 | 想做小体积时用，但务必按 USB-C 规范加 Rp；或直接上 TUSB320 类 CC 控制器 |
| USB-C 母座做 device（UFP） | 用于烧录/调试 | 不能接手机当 host | 若做"双口"方案用（§4.4 架构 B） |

USB-C 电阻取值的依据：TI TUSB320 数据手册（USB Type-C CC 逻辑芯片）中 "Legacy Host Adapter" 图给出 **Rp = 56 kΩ ±5%（source，默认 USB 电流档）、Rd = 5.1 kΩ ±10%（sink）**。<https://www.ti.com/lit/ds/symlink/tusb320.pdf>

#### 4.3.2 VBUS_EN / VBUS_DET / OVCUR 三根可选 GPIO

数据手册 §12.7.3.10（p1070）：控制器可接三路 GPIO——**VBUS enable**（host 模式开 VBUS，`SIE_CTRL.VBUS_EN` bit11 / `USB_PWR.VBUS_EN`）、**VBUS detect**（device 模式检测 VBUS，`SIE_STATUS.VBUS_DETECTED`）、**VBUS overcurrent**（`USB_PWR.OVERCURR_DETECT`）。官方明说"不接也能用"：host 可以常供电，靠 DP/DM 上拉判断插入。

GPIO 复用：这三路是 GPIOn **F10** 功能（`usb_muxing_vbus_en` / `usb_muxing_vbus_detect` / `usb_muxing_overcurr_detect`），在多个 GPIO 上可选（如 GPIO2/GPIO14 可作 VBUS_EN，GPIO1/GPIO13 可作 VBUS_DET，GPIO0/3/12 可作 OVCUR——具体以数据手册 §9.4 GPIO 功能表为准）。

推荐接法（v1）：

    5V_IN ──[ 负载开关/限流开关 ]── VBUS(USB-A pin1)
                  │ EN        │ FAULT(开漏)
                  │           └──(可选) GPIO(OVCUR)  ← 注意极性/上拉
                  └── GPIO(VBUS_EN, F10)
    VBUS(USB-A) ──[ 100k/100k 分压 ]── GPIO(VBUS_DET)   ← 5V 不能直连 3.3V GPIO
    D+ / D- ──[ 27Ω 串阻，靠芯片 ]── USB_DP / USB_DM ──[ USBLC6-2SC6 ESD ]── 座子

要点：
- **VBUS_EN**：控制负载开关 EN，软件在 host 初始化时置位。若省掉这颗开关、VBUS 常带电，则**手机插入瞬间的浪涌**没有限流，且"拔出再插"无法断电复位；不建议省。
- **VBUS_DET**：host 模式其实不需要；**若要用，必须分压**（RP2350 GPIO 绝对最大 3.63 V，5 V 直连会损坏）。
- **OVCUR**：负载开关 FAULT 多为**开漏低有效**，而控制器输入极性需按 `USB_PWR.OVERCURR_DETECT` 定义处理；【待验证】最稳妥的做法是**不用硬件 OVCUR**，软件轮询 FAULT 或直接依赖限流开关自恢复。
- **27 Ω 串阻**：官方明确要求（《Hardware design with RP2350》§5.1："these I/Os do require 27 Ω series termination resistors (R7 and R8), placed close to the chip"），且**不需要**外部 1.5 k 上拉/15 k 下拉（PHY 内置，host 模式自动弱下拉）。
- **差分阻抗**：目标 90 Ω。官方 2 层 1 mm 板给的经验值：线宽 0.8 mm、间距 0.15 mm；4 层板请用阻抗计算器按叠层算，并保证 D+/D- 下方**完整地平面**、不打孔换层、不跨分割。
- **ESD/共模/地**：座子侧放 USBLC6-2SC6（D+/D- 与 VBUS），器件**靠近连接器**、走线先过 ESD 再进芯片；FS 12 Mb/s 一般不需要共模电感（EMC 不过再补，选 USB 专用型）；USB 座外壳与板 GND 直接连即可（严格场合用 1 MΩ ∥ 4.7 nF）。

#### 4.3.3 5 V 电源树（板子自身供电 + 对外 VBUS）

    USB-C 5V 输入(受电, CC 各 5.1kΩ Rd) ──┬── 3.3V LDO/Buck (≥600 mA) ── RP2354 + OLED + 逻辑
                                          │
                                          ├── 负载开关(限流 0.5–1.5 A) ── USB-A VBUS(对外 5V)
                                          │
                                          └── (可选) CDP/BC1.2 控制器 ── D+/D- 握手

- **输入侧**：若用 USB-C 做电源输入，CC1/CC2 各接 **5.1 kΩ Rd** 到 GND，充电器才会输出 5 V；建议加 TVS + 自恢复保险丝（或理想二极管）防反灌。
- **对外 VBUS**：手机按 USB 2.0 规范最多取 **500 mA**（SDP）；限流开关设 0.5–1 A 即可。**不要**把输入 5 V 与对外 VBUS 直接短接——手机拔插/短路会把整板拉垮，且无法软件断电复位。
- **BC1.2（要不要做）**：不做的话手机把端口识别为 **SDP**（标准下行口），能枚举、能数据、充电限 500 mA——对"手持调试工具"完全够用。做了 **CDP**（D+/D- 各 0.6 V 握手，需 TPS2543 类芯片）手机才会以更大电流充电，同时保留数据；**DCP**（D+/D- 短接）只充电、**不能数据**，千万不要误做成 DCP。依据：TI TPS2543 数据手册（"D+/D– CDP/DCP Modes per USB Battery Charging Specification 1.2"、"SDP 500 mA/900 mA"）。<https://www.ti.com/lit/ds/symlink/tps2543.pdf>
- **【推断】手机侧行为**：Android 机在 SDP 上通常正常枚举并允许数据；但个别机型在"仅充电"策略或低电量时会延迟枚举——**这是 §8.3 必须用真机先验证的第一件事**。

### 4.4 烧录/调试通道（单 USB 控制器的硬约束）

RP2350 只有**一个** USB 控制器（§2.2），host 模式占用后就不能同时当 USB device。三种架构：

| 架构 | host 口 | 烧录/调试 | 优点 | 缺点 | 建议 |
|---|---|---|---|---|---|
| **A. 原生 host → USB-A** | 原生控制器（USB_DP/DM 直连 A 座） | **SWD（probe-rs）+ UART 测试点**；BOOTSEL 按键仍可用于"应急"（把 A 座当 device 用需要 A-to-A 线，不规范，不建议） | 最简单、host 性能/稳定性最好 | 每次升级要接调试器（或用 UART bootloader） | ✅ **v1 推荐** |
| B. 原生 device（USB-C） + **PIO-USB host**（GPIO12/13 → A 座） | PIO（占 1 PIO + 3 SM） | USB-C 原生：UF2/PICOBOOT/CDC 串口，插电脑即可 | 开发体验最好，可双角色 | 依赖 PIO-USB 在 RP2350 上的稳定性（§2.4 有已知问题）；需外部 **15 kΩ 下拉**、且该口**不能有 1.5 k 上拉** | 若你重视"插电脑就能烧"选这个 |
| C. 单 USB-C 口 + **USB mux**（如 TS3USB30E）切换 DP/DM 到"调试座"或"host 座" | 原生控制器，mux 切到 A 座 | mux 切到调试 C 座 | 一个控制器两种用途 | 多一颗 mux + 控制 GPIO，SI 更复杂 | 进阶方案 |

真实案例警示：Waveshare RP2350-USB-A 板的 USB-A 口默认按 **device** 设计（D+ 有 1.5 k 上拉），改 host 必须"拆 R13（1.5 k 上拉）+ 加两颗 15 k 下拉到 D+/D-"，否则 host 栈检测不到任何设备。来源：<https://github.com/THLN33/RP2350-USB-A>（README 的 Hardware Modification 一节）。**自画板时务必确认 host 口没有 1.5 k 上拉**；用原生 host 时也不需要外部下拉（内置）。【待验证】若走 PIO-USB 路线，则必须外加 15 k 下拉（PIO 模拟的 PHY 没有内置下拉）。

量产/测试建议（自用可简化）：SWD 测试点（SWDIO/SWCLK/GND/3V3）+ UART TX/RX 测试点 + BOOTSEL/RUN 可探针短接；首件用 `probe-rs` 烧 blink 验证晶振/USB/OLED。

### 4.5 板级风险清单

| 风险 | 等级 | 说明与对策 |
|---|---|---|
| **QFN-60/80 手焊** | 高 | 官方原话："it is **very nearly impossible** to solder the QFNs without specialist equipment"；0.4 mm pitch + 散热焊盘必须**钢网 + 热板/回流焊**。对策：直接嘉立创/JLCPCB SMT 贴片（含 QFN 与 0402），不要手焊 |
| **USB 差分信号完整性** | 中 | 90 Ω 差分、27 Ω 串阻靠芯片、完整参考地、ESD 靠座子；4 层板（信号-地-电源-信号）优于官方 2 层 |
| **VBUS 反灌/过流/5 V 直连** | 中 | 输入 5 V 与对外 VBUS 之间必须有限流开关 + 防反灌；VBUS_DET/OVCUR 若接 5 V 信号必须分压/电平转换，否则烧 GPIO |
| **ESD/EMC** | 低-中 | 自用设备可放宽；座子加 ESD 阵列、外壳接地即可。认证（CE/FCC/CCC）自用可忽略，**但不得对外销售**（无线电/EMC 法规） |
| **晶振/VREG 布局** | 中 | 必须照抄官方 Minimal：VREG 电感方向、PGND 回流、VREG_AVDD RC 滤波；晶振走线短、15 pF C0G |
| **RP2354 缺货/假货** | 中 | 只从 LCSC/授权代理买；备 B 方案（RP2350A + 外部 flash，同板兼容） |

### 4.6 原理图分块与 BOM 建议（含成本估算）

| 分块 | 关键器件 | 1 片 | 10 片 | 100 片 |
|---|---|---|---|---|
| MCU | RP2354A（QFN-60） | $1.27 | $1.25 | $1.22 |
| 时钟 + 内部 1.1 V | ABM8-272-T3 + 2×15 pF + 1 kΩ + 3.3 µH + 3×4.7 µF + 33 Ω | $0.80 | $0.70 | $0.60 |
| 3.3 V 电源 | LDO/Buck + 2×10 µF | $0.25 | $0.20 | $0.15 |
| 去耦/阻容 | ~40× 0402 | $0.40 | $0.30 | $0.25 |
| USB host 口 | USB-A 座 + 负载开关 + USBLC6 + 27 Ω×2 | $0.60 | $0.50 | $0.40 |
| 电源输入 | USB-C 座 + 5.1 k×2 + TVS/保险丝 | $0.25 | $0.20 | $0.15 |
| 调试 | SWD 排针/JST + UART 测试点 + BOOTSEL/RUN 按键 | $0.25 | $0.20 | $0.15 |
| 人机 | 0.96" SSD1306 I2C OLED 模块 + 5 按键 + 蜂鸣器 | $2.30 | $2.00 | $1.70 |
| PCB | 4 层 100×70 mm（5 片起） | $8–20/5 片 | 摊薄 | 摊薄 |
| SMT 贴片 | 嘉立创经济型（含钢网/工程费） | 约 $10–25（首单） | 摊薄 | 摊薄 |
| **合计（推断）** | | **约 $25–40/片** | **约 $13–18/片** | **约 $7–11/片** |

【推断】以上为"自用、不含认证、不含外壳/电池"的粗算；100 片量级主要成本被 SMT 工程费与 OLED 模块吃掉。若要压到 $10 以下，需把 OLED 换成裸屏（COG）+ FPC、按键改硅胶、去掉负载开关（不推荐）。

---

## 5. 存储与内存预算

### 5.1 flash（RP2354 内置 2 MB）

| 项 | 估算 | 说明 |
|---|---|---|
| 启动块（ImageDef + 向量表） | < 8 KB | RP2350 无 boot2；仅需前 4 KB 内含合法 ImageDef（.start_block） |
| 应用代码（Rust，opt-level=s + LTO） | 250–450 KB | 协议栈 + USB host 栈 + UI + HAL；【推断】以 cotton + RTIC 2 + rp235x-hal + embedded-graphics 计 |
| 中文/ASCII 点阵字库 | 32 KB（ASCII+少量汉字，12/16 px）～256 KB（GB2312 一级字库 16×16） | 见 09 文档；建议只取用到的字 |
| 图标/位图 | 8–32 KB | 128×64 单色 |
| 日志/配置 | 32–128 KB | 环形日志 + 校准数据；也可只放 RAM |
| **合计** | **约 400 KB–900 KB** | 2 MB 余量充足，甚至可留 OTA 双区（2×512 KB） |

对比：若用 RP2350A + **外部 16 MB W25Q128**，flash 不再是瓶颈但多 6 条 QSPI 走线 + 一颗器件 + 一个贴片位；**RP2354 的 2 MB 对本项目是"刚好够且省事"**。若将来要放完整中文字库（GB2312 全集 6763 字 × 16×16 ≈ 216 KB）也毫无压力。

### 5.2 SRAM（520 KB）

| 项 | 占用 | 说明 |
|---|---|---|
| USB 控制器 DPSRAM | 4 KB（硬件独立） | 不算在 520 KB 里；host 模式用其中 3840 B 数据区 |
| USB host 栈（cotton/TinyUSB） | 8–24 KB | 设备树 + 端点缓冲池 + 类驱动 |
| 协议层缓冲 | 8–16 KB | bulk 传输最大包 64 B，但会话/固件包可用大缓冲；按 16 KB 留 |
| Type2Auth/加密 | 4–16 KB | AES/SHA 上下文与表（取决于算法实现） |
| UI framebuffer | **1 KB**（128×64/8） | 双缓冲 2 KB；embedded-graphics 可直写 |
| 字库缓存/菜单状态 | 2–8 KB | |
| 栈 + 堆（alloc） | 32–64 KB | 建议核心库 no_std+alloc，堆留 48 KB |
| **合计** | **约 60–130 KB** | **520 KB 非常宽裕**（约 4–8 倍余量） |

⇒ 内存不是瓶颈；**唯一要注意的是 USB host 栈的缓冲池配置**（cotton 用静态池，需按设备数/端点调参），以及避免把大数组放在栈上（默认线程栈 8 KB 级别）。

---

## 6. 先例调研：有没有 MCU/Pico 做 USB host 与 Android/au 手机 SERI/Type2Auth 交互的公开案例

| 类别 | 找到什么 | 结论 |
|---|---|---|
| MCU 做 USB host | 大量：TinyUSB host 例程（CDC/MSC/HID）、Pico-PIO-USB、Waveshare RP2350-USB-A 板、THLN33 的 HID→UART 桥、cotton 的 RP2040/RP2350 MSC/HID/以太网示例 | ✅ 成熟 |
| MCU 接 Android 手机做数据 | 有（USB 转串口、ADB 类、CDC-ACM 类实验），但多为"手机当 host、MCU 当 device"；**反向（MCU 当 host、手机当 device）** 的公开项目少见 | ⚠️ 少 |
| **au 手机 SERI / Type2Auth** | **未找到任何公开的 MCU 实现**；现有实现只有 auRokku 本身（桌面 libusb）与其前身 | ❌ 没有先例 |
| RP2350 原生 host 的实测报告 | TinyUSB issue #3875 的作者在 RP2350B 上跑通了 hub + 键盘 + U 盘（除 control IN 长包 bug）；issue #4310 同样在 RP2350 板上 | ✅ 有人真的在跑 |

**结论：平台能力有先例，具体到"au 手机 SERI 协议"没有先例**——所以 §8.3 的真机验证不可跳过：要验证的不是"RP2350 能不能当 host"，而是"这台手机在自制 host 口上愿不愿枚举、描述符多长、SERI 端点是否与桌面端一致"。

---

## 7. 备选方案对比（若 RP2354 路线受阻）

| 方案 | USB host 能力 | OLED/按键 | 成本（单板） | 开发栈 | 评价 |
|---|---|---|---|---|---|
| **RP2354A 原生 host**（本文推荐） | FS/LS host、hub、15 int EP（官方） | I2C/GPIO 充裕 | $10–40 | Rust（cotton）或 C（TinyUSB） | ✅ 综合最优；风险在 TinyUSB host bug 与自制板 |
| RP2350B + PIO-USB | 原生 host + PIO 第二口（可双角色） | 同上 | 同上 | C/Rust + PIO | 需要"边当 host 边当 USB 串口"时选；PIO-USB 在 RP2350 上未文档化、有丢包报告 |
| ESP32-S3 | USB-OTG FS，TinyUSB 有 dwc2 host 驱动【待验证具体版本/稳定性】 | 充裕（WiFi/BLE 额外） | $5–15 | C（ESP-IDF）/ Rust（esp-hal，host 支持弱） | 便宜、生态大，但 host 侧文档与例程不如 RP2 清晰 |
| STM32F4/F7（OTG FS/HS） | 硬件 OTG host，TinyUSB hcd_dwc2 成熟；F7 可 HS | 充裕 | $5–20 | C 为主（Rust host 栈稀缺） | host 最"正统"，但要放弃 Rust 核心（FFI 或重写） |
| Linux SBC（Zero 2W 等） | 完整 USB host（含 HS） | HDMI/DSI 或 I2C OLED | $15–25 + 电源 | 直接复用现有 Rust 核心（libusb） | 软件零移植，但体积/功耗/启动时间不适合手持 |

**决策逻辑**：如果第一优先级是"最快拿到能用的手持设备"→ Linux SBC 或"手机 + OTG 线 + 现有工具"其实更省事；如果第一优先级是"低功耗、单芯片、可长期维护的硬件产品"→ RP2354 路线值得投入。**不建议**为了 USB host 换 STM32（Rust 侧要重写核心库，代价远大于收益）。

---

## 8. 结论、风险与最小验证实验

### 8.1 结论

**【有条件可行】**，且条件都可满足：

1. **平台层**：USB host 是 RP2350/RP2354 的官方能力（§12.7.1、§12.7.4），不是社区 hack；速率（FS 12 Mb/s）与协议（control+bulk）完全匹配 auRokku 的需求。
2. **软件层**：Rust 主线（rp235x-hal 0.4 + RTIC 2 + cotton-usb-host **git pin**）可行且能复用核心库；C 基线（pico-sdk + TinyUSB 0.20.x）作为对照与兜底。
3. **硬件层**：RP2354A 省掉外部 flash，2 MB flash + 520 KB SRAM 富余；难点是 QFN 贴装与 USB 差分走线，**用 SMT 贴片 + 4 层板即可控**。
4. **真正的未知**不在平台，而在"**au 手机在非 PC host 上的枚举行为**"与"**TinyUSB RP2 HCD 的长 control IN bug**"——两者都能用一周内的实验证伪/规避。

### 8.2 关键风险（按优先级）

| # | 风险 | 概率/影响 | 缓解 |
|---|---|---|---|
| R1 | TinyUSB RP2 HCD #3875（长 control IN 只收第一包）导致枚举失败 | 中 / 高 | 固定 TinyUSB **0.20.x**（issue 说 0.20 正常）；或打补丁；或用 cotton（自有 control 实现）；实测时优先验证"读完整配置描述符" |
| R2 | 手机在自制 host 口上不枚举（充电策略/BC1.2/线材） | 中 / 高 | 用真机 + 多种线（A-to-C）；先 SDP；必要时上 CDP（TPS2543）；保留 VBUS 断电重插能力 |
| R3 | QFN + USB SI 一次打样不成功 | 中高 / 中 | 4 层板、照抄官方 Minimal、SMT 贴片、27 Ω + ESD + 90 Ω；v1 只做最小系统 |
| R4 | cotton-usb-host **0.3.0 无 RP2350 支持（必须 git pin/vendor）** + 无 SET_INTERFACE + 单人维护 | 中 / 中 | 依赖写 git = https://github.com/pdh11/cotton, rev = 6b43803e，或 vendor 进 third_party/；确认 SERI 接口 alt=0；必要时提 PR 或退回 TinyUSB |
| R5 | 单 USB 控制器导致调试体验差 | 高 / 低 | v1 用 SWD + UART；或架构 B（原生 device + PIO host） |
| R6 | RP2354 采购/缺货 | 中 / 低 | 同板兼容 RP2350A + 外部 flash；只走授权渠道 |

### 8.3 最小验证实验（建议按顺序，1 周内可完成）

**E0（0.5 天，$15 内）：先验证"手机愿意被枚举"**。买 **Raspberry Pi Pico 2** + USB-A 母座转接板 + 5 V 电源，跑 **Pico-PIO-USB** 的 host 例程（GPIO12/13 + 15 k 下拉 + A 座 VBUS 5 V）——这是接线最可控、不依赖 Pico 板载 USB-C 走线的做法。插 U 盘/键盘确认通路，再插 **au 手机**，用 `tuh_descriptor_get_*` 打印 device/config/string 描述符。
→ 验证：手机是否枚举、配置描述符长度（≥3 包会踩 R1，但 PIO 路径不受 TinyUSB RP2 HCD 影响）、SERI 接口号/端点/alt。

**E1（1 天）：验证原生 host 路径（架构 A 的关键）**。Pico 2 的原生 USB 只出了 USB-C 口，要当 host 需给 VBUS 注入 5 V 并让手机经 A-to-C 接入（可用带 VBUS 注入的 USB-C 分线板/拆线注入；**这种接法不规范，只用于可行性验证**）。跑 pico-examples `usb/host/host_cdc_msc_hid`（TinyUSB 0.20.0 与 0.21.0 各跑一遍）对同一台手机做枚举与描述符读取。
→ 验证：R1（#3875 是否触发）、原生 host 与 PIO host 的枚举成功率差异，据此决定 v1 走架构 A 还是 B。

**E2（2–3 天）**：把 08 文档抽出的 no_std 核心库在 Pico 2 上跑通"枚举 + 打开 SERI 端点 + 一条真实命令"（可先用 C 写 USB 层、Rust 做协议，快速验证协议在真机上成立）。
→ 验证：核心库 no_std 化是否真的可行（这是 08 的任务，但 E2 是端到端证据）。

**E3（1–2 周）**：画 **v1 定制板**（RP2354A + USB-A host + SWD + UART + OLED I2C + 5 键 + 蜂鸣器 + 5 V 输入/限流），4 层、SMT 贴片，5 片起。
→ 验证：R3（SI/贴装）、电源树、VBUS 反灌；此时再决定是否加 CDP、是否加第二 flash。

**E4（可选）**：若架构 B 更好，v2 改为"原生 device（USB-C 烧录/串口）+ PIO-USB host（USB-A）"，或加 USB mux。

### 8.4 工作量估算【推断】

| 阶段 | 内容 | 人日（熟练/兼职） |
|---|---|---|
| 平台 spike（E0–E2） | 工具链 + host 栈 + 真机枚举 | 3–6 |
| 核心库 no_std 化 | 见 08 文档 | 5–10 |
| UI（OLED+按键+菜单） | 见 09 文档 | 5–8 |
| v1 硬件设计/打样/调试 | 原理图 + PCB + 贴片 + bring-up | 8–15 |
| 整机联调/稳定性 | 长时间跑、异常恢复、日志 | 5–8 |
| **合计** | | **约 26–47 人日**（兼职 2–3 个月） |

---

## 9. 附：引用来源（按主题）

- **Raspberry Pi 官方**：RP2350 数据手册（RP-008373-DS）<https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf>（§5.2 启动/镜像、§12.7 USB、§14.3 flash-in-package、§D.11 errata）；Product Brief <https://datasheets.raspberrypi.com/rp2350/rp2350-product-brief.pdf>；Hardware design with RP2350（RP-008279-DS）<https://datasheets.raspberrypi.com/rp2350/hardware-design-with-rp2350.pdf>；RP2350A/B Minimal KiCAD <https://pip.raspberrypi.com/documents/RP-010328-CA> / <https://pip.raspberrypi.com/documents/RP-010329-CA>；产品页 <https://www.raspberrypi.com/products/rp2350/>
- **pico-sdk / 例程**：<https://github.com/raspberrypi/pico-sdk>（2.3.1；tinyusb submodule = 0.18.0）；USB host 例程 <https://github.com/raspberrypi/pico-examples/tree/master/usb/host>
- **TinyUSB**：RP2 HCD 源码 <https://github.com/hathach/tinyusb/blob/master/src/portable/raspberrypi/rp2040/hcd_rp2040.c>；0.17.0（RP2350 支持）<https://github.com/hathach/tinyusb/releases/tag/0.17.0>；0.21.0（RP2 重构）<https://github.com/hathach/tinyusb/releases/tag/0.21.0>；issue #3875 <https://github.com/hathach/tinyusb/issues/3875>、#4310 <https://github.com/hathach/tinyusb/issues/4310>
- **PIO-USB / 社区**：Pico-PIO-USB <https://github.com/sekigon-gonnoc/Pico-PIO-USB>（#141/#181/#189/#203）；THLN33/RP2350-USB-A（host 硬件改法）<https://github.com/THLN33/RP2350-USB-A>
- **Rust 生态**：cotton-usb-host <https://github.com/pdh11/cotton> / <https://crates.io/crates/cotton-usb-host> / <https://docs.rs/cotton-usb-host>；embassy <https://github.com/embassy-rs/embassy>；probe-rs RP235x 目标 <https://github.com/probe-rs/probe-rs/blob/master/probe-rs/targets/RP235x.yaml>；rp235x-hal <https://crates.io/crates/rp235x-hal>
- **USB 电气/充电**：TI TUSB320（USB-C CC、Rp 56k/Rd 5.1k）<https://www.ti.com/lit/ds/symlink/tusb320.pdf>；TI TPS2543（BC1.2 SDP/CDP/DCP）<https://www.ti.com/lit/ds/symlink/tps2543.pdf>；USB 规范库 <https://www.usb.org/document-library>
- **价格/库存**：LCSC RP2354A <https://www.lcsc.com/product-detail/C41378174.html>、RP2354B <https://www.lcsc.com/product-detail/C39843328.html>

> 本文所有价格/库存为 2026-11-04 查询值，随时变动；标【推断】的成本与工时为工程估算，非报价。文中"§12.7"等章节号均指 RP2350 数据手册。
