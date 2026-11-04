# 08 · 核心库可移植性：main 的 auroku 协议层能否抽成 no_std core

> 作者：teammate **core-analyst**（只读分析）。生成于 2026-11-04。
> 分析对象：main 工作树 C:/Users/Administrator/Documents/proj/aurokku-rs，HEAD = **e594539**（其上仅文档提交；git diff --stat b219bd5..HEAD 为空，故本文所有行号对 v0.3.1 产品代码成立）。
> 方法：读 src/lib.rs(3139 行) / src/driver.rs(719) / src/bin/auroku-gui/worker.rs(246) / Cargo.toml / README.zh-CN.md；用 pwsh 做行区间与模式计数；从本机 cargo registry 反查依赖的 no_std 元数据。**未改任何产品代码，未跑 cargo**。
> 标记：**【事实】**=代码/依赖源码可直接核对；**【推断】**=由代码推导未实测；**【待验证】**=依赖平台或硬件，交由 09/10 文档。
> 读者：决定「值不值得做、怎么切、风险与工作量」的工程决策者。

---

## 0. 结论摘要（10 条）

1. **结论：值得做，但可移植的是「协议语义层」，不是「设备控制器层」。** lib.rs 的 2574 代码行里约 **900 行（36%）**是协议/编解码/鉴权/编排逻辑，可几乎原样迁进 no_std；约 **1390 行（54%）**是 rusb 绑定、CLI、JSON、宿主枚举；src/driver.rs 719 行 100% 是 Windows 驱动栈，MCU 上整体丢弃。
2. **【事实】协议层完全不用 control transfer。** 全仓只有 4 个数据传输调用：read_bulk / write_bulk / read_interrupt / write_interrupt（lib.rs:650-651、665-666）。任务书假设的 bmRequestType / bRequest / wIndex 在协议里**不存在**；控制传输只发生在 libusb 内部（取 iSerialNumber lib.rs:819、claim/set_alt lib.rs:925-930）。
3. **【事实】因此传输抽象只需要 4 个方法 + 2 个辅助**（write / read / delay / 单调毫秒时钟）。12 个 Task 变体全部只依赖这 4+2 个原语 → **可移植性由传输抽象决定，而不是由操作决定**。
4. **【事实】0x04/0x84 只是默认值**（lib.rs:29-31），语义是 **EP4 OUT / EP4 IN**；Bulk 还是 Interrupt 由端点描述符在运行时判定（lib.rs:331-367 + 316-328），与协议序列无关。固件只要按描述符找到同一对端点即可。
5. **【事实】Type2Auth 是纯 MD5 + 字节置换 + 位反转**（lib.rs:170-296）：无 DES/AES、无随机数、无时间依赖。md-5 0.10.6 本身就是 #![no_std]（本机 registry md-5-0.10.6/src/lib.rs:26），default-features=false 即可在 core 用。
6. **【事实】阻塞模型可 1:1 映射到 embassy**：只有 4 处 Instant::now()（lib.rs:679/740/1050/1167）与 2 处 thread::sleep(50ms)（1223/1268）；其余都是「绝对 deadline + 500 ms 分片读」。
7. **【事实】协议路径无 panic、无 unwrap**：非测试代码只有 2 处 expect（lib.rs:812/818，位于 connect 之前不可达），0 处 unwrap()；数组索引点均先判长度（1196/1323/1486/1505/1657）。这对 MCU（panic=挂死）是重大利好。
8. **【事实】uimlk 认证是无条件的**（lib.rs:1261），因此「读 UIM 锁 / ICCID / status」都必须先拿到 SN → 必须能读 iSerialNumber（lib.rs:816-830）或手工输入 SN（lib.rs:832-848）。这是 MVP 范围的硬约束。
9. **建议切法**：新建 crates/auroku-core（no_std，feature std/alloc/serde），用 trait 承载传输（**async / sync 二选一，见 §6.3b**，取决于 06 的 C ABI 决策）；桌面 crate 保留 rusb/clap/JSON 并把 core 结果转成现有 JSON 形状（GUI 有 **36 处**字段访问，app.rs，必须逐字保持键名）。
10. **【推断】工作量**：core 拆分 11.5 人日（含测试迁移），固件传输实现 5–10 人日；MVP（probe seri/scdp + readpin + exit）≈ **12–18 人日**，完整 12 op ≈ **20–28 人日**，不含 OLED UI。最大风险不在协议，在固件 USB host 栈与热插拔（§7 R7/R8）。

---

## 1. 分层地图：lib.rs 的五类内容与真实可移植内核

### 1.1 五层归属（行号为主归属，区间无重叠；覆盖 3112/3139，余下为文件头与分节注释）

| 层 | lib.rs 行区间 | 总行 / 代码行 | 关键类型与函数（行号） | 可移植性 |
|---|---|---|---|---|
| **L1 纯协议 / 编解码 / 鉴权** | 118-314 | 195 / **156** | encode_nibbles(123)、decode_nibbles(133)、decode_lockcode_digits(148)、parse_sn_to_type2auth(170)、bitrev8(223)、derive_type2auth_key(228)、md5_digest(248)、compute_type2auth_request(260)、find_subslice(309) | **可原样迁 core**（仅 hex_spaced 302 需 alloc） |
| **L2 USB 主机 I/O（rusb）** | 513-597、599-758、760-935、1144-1157 | 435 / **372** | AuRokkuDevice 字段(517-541)、ensure_handle(633)、read_endpoint_in(647)、send(655)、recv(678)、drain(716)、read_until(734)、open_device(765)、connect(868)、close(1145) | **换实现**：逻辑保留、实现换人 |
| **L3 任务 / CLI / JSON 编排** | 36-77、369-511、1159-1815、1818-1850、2175-2235、2237-2900 | 1596 / **1335** | AuRokkuError(42)、DeviceConfig(380)、Task(422)、ProbeKind(459)、BatchProfile(489)、会话与操作 1159-1815、prepend_fields(1822)、run_task(2179)、dispatch_task(2220)、Cli(2302)、print_human_report(2427)、cli_main(2814) | **分裂**：1159-1815 的 530 行=协议语义可移植；CLI/JSON 571+31 行=外壳 |
| **L4 设备扫描 / 枚举** | 316-367、937-1142、1856-1887、2091-2173 | 373 / **316** | XferKind(318)、find_interface(331)、inspect_usb(938)、read_probe_response(1048)、probe(1079)、DeviceSummary(1858)、scan_devices(2096) | **对半**：probe 的 SERI/SCDP 主动探测可移植，描述符枚举归固件 |
| **L5 宿主专属** | 79-116、1889-2090 | 240 / **183** | open_failure(85)、From<rusb::Error>(106)、sysfs 名字回退(1899-1935)、Windows 注册表/PowerShell(1964-2090) | 全删（另加 driver.rs 719 行、permissions.rs 509 行） |
| L0 测试 | 2902-3139 | 238 / **207** | 19 个 #[test]（其中 3 个平台相关：2929/3061） | 约 150 行可迁 |

### 1.2 真正可移植的「内核」= 三个纯函数族 + 一个操作序列集合

**【事实】** 把 L1(156) + L3 的协议语义段 1159-1815(530) + device_summary(27) + dispatch_task(35) + material 等 591-597(15) + 错误枚举便携部分(≈40) ≈ **803 代码行**；把周边工具（hex_spaced / find_subslice / parse_sn 边界）也算上约 **900 行**（= lib.rs 代码行的 **36%**、文件行的 **29%**）。

**这 900 行的共同特征**：只依赖「字节进 / 字节出 + 时间 + 日志」四件事，无一行动作涉及 USB 句柄、文件、进程、环境变量。

反向看 **不可移植的 1390 行**（= 代码行 54%）由三类构成：rusb 调用与句柄生命周期（≈490 行）、serde_json 结果构造与打印（≈200 行）、clap/GUI 外壳与宿主枚举（≈700 行）。

**【事实/已验证，交叉引用】** 同目录 **10-core-nostd-spike.md** 已把 13 个协议单测逐字迁到 no_std 并在 thumbv8m.main-none-eabihf 上 check/build 通过；其依赖结论（md-5 关 std、hex 需 alloc）与本文 §5.4 一致，可作为「§1.2 的 900 行确实可搬」的编译级证据。本文补的是它未覆盖的部分：传输抽象怎么切、桌面 JSON 怎么保、超时/内存/工作量与风险。

### 1.3 一句话分层结论

> 现在的 AuRokkuDevice 把「协议状态机」和「传输实现」写在同一个 impl 里（513-1816 共 1304 行）。移植的本质动作是**在这 1304 行中间横切一刀**：1159-1815 进 core，599-1142 换实现。切完后 core 不需要知道 USB 存在。

---

## 2. 操作清单与 USB 传输时序

### 2.1 底层只有 5 个原语（全部在 lib.rs:599-758 + 1048-1076）

| 原语 | 行号 | 语义 | 超时 / 分片 | 移植要点 |
|---|---|---|---|---|
| send | 655-676 | 按 ep_out_kind 调 write_bulk/write_interrupt | **固定 2000 ms**（663），短写→RuntimeError(669-674) | 单包即完成：最长发送 17 B（认证包）、最长接收 21 B，均小于 FS bulk 单包 64 B |
| recv | 678-713 | 累加 rx_buffer 到 expected_len | deadline=now+timeout(679)，每次 read **500 ms** 分片(690) | 唯一需要「单调时钟」的地方 |
| drain | 716-732 | 清本地缓冲 + 最多 20 次 read | 20×timeout_ms；调用处一律 50 ms（933/1107/1220/1224） | 最坏 1 s；吞掉非 Timeout 错误（729 的 _ => break）→ 固化时必须改成上抛 |
| read_until | 734-758 | 逐字节读到终止符 | deadline(740)，limit 触发 RuntimeError(755) | 实际是 recv(1) 循环，1 B/次，效率无妨 |
| read_probe_response | 1048-1076 | 最多 512 B，超时即收 | 分片 clamp(1,200) ms(1055)，有数据后超时即 break(1066-1070) | 唯一用 f64 的地方之一 |

**【事实】端点与传输类型的完整逻辑**：
1. 默认 DEFAULT_EP_OUT=0x04、DEFAULT_EP_IN=0x84（lib.rs:29-31），CLI 默认串 "04"/"84"（2332-2337），并校验方向位（2837-2844）。
2. find_interface(331-367) 遍历**所有配置 × 所有接口 × 所有 alt setting**，按 bEndpointAddress 精确匹配，记录两端的 transfer_type；**先命中者胜**（356-363）。
3. connect(868-935)：detach kernel driver(921-923) → claim_interface(925) → 若 alt≠0 再 set_alternate_setting(929) → drain(50)(933)。
4. 0x04/0x84 → 端点号 4，OUT/IN 各一。**Bulk/Interrupt 由描述符决定，代码不假设**（XferKind 316-328）。

### 2.2 12 个 Task / 15 条操作的报文序列

记号：TX→ 发送、RX←n 期望 n 字节并校验；所有 RX 的单次 deadline = DEFAULT_TIMEOUT 3 s（lib.rs:27），除注明者。

| 操作（Task） | 入口（行号） | 报文序列 | 会话前提 | 最坏耗时※ |
|---|---|---|---|---|
| status | read_status 1730 | base: 1B→RX←5；DC 00→read_until(0xAA,≤256)；E7→RX←1；auth uimlk（52→RX←17，53+16nib→RX←2）；54→RX←10；59→RX←21；56→RX←1 | base→uimlk；**恒需 SN** | **≈35 s**（--type2 再 +6 s） |
| readuimlk | 1341 | enter_uimlk；54→RX←10；56→RX←1 | 需 SN | ≈20 s |
| clearlk | 1470 | enter_uimlk；54→RX←10；57→RX←2；54→RX←10；56→RX←1 | 需 SN | ≈26 s |
| writelk | 1443 | 同 clearlk，改发 72,ctl,+8nib（共 10 B）→期望 [72,01] | 需 SN | ≈26 s |
| readiccid | 1475 | enter_uimlk；59→RX←21；56→RX←1 | 需 SN | ≈20 s |
| readpin | 1520 | DC 00→read_until(0xAA,≤256,**1 s**)；失败→enter_base 后重试（read_until 3 s） | base（可不认证） | ≈9 s |
| resetpin | 1560 | DC 01→RX←2,1 s；失败→enter_base 重试；再回读 DC 00 校验 PIN=1234 | base | ≈18 s |
| maintenance_bit | 1612 | enter_base；E1→RX←1；auth **4000**；72,ctl→RX←2 | base + auth 4000 | ≈17 s |
| batch basic | 1680 | enter_base；1B→RX←5；DC 00→read_until | base | ≈11 s |
| batch uimlk | 1680 | enter_base；E7→RX←1；auth uimlk；54→RX←10；59→RX←21 | base→uimlk | ≈20 s |
| probe list | 1079 | **无报文**，仅描述符枚举（不 claim；1101-1105 会拒绝主动探测） | 不需 SN | <50 ms |
| probe seri | 1079 | drain(50)；1C 05→≤512 B（回显比较 1114）；若匹配且 vid≠0x0FCE 再补 D8(1122-1125) | claim，不需 SN | 1 s（可配） |
| probe scdp | 1079 | drain(50)；8E→≤512 B | claim，不需 SN | 1 s |
| exituimlk | 1797 | enter_uimlk；56→RX←1 | 需 SN | ≈17 s |
| exit | 1797 | D8（仅 vid≠0x0FCE，1266-1279） | 无需 SN（但 connect(true) 仍要，2210） | ≈1 s |

※ 最坏耗时 = 该路径上所有「deadline 上限」与 drain 上限相加（【推断】由代码常量累加，非实测）。典型成功路径应 <0.6 s，**【待验证】**需上机实测。

### 2.3 会话状态机（4 个字节级握手）

| 阶段 | 函数 | 动作与校验 |
|---|---|---|
| 进入 base | enter_base 1219-1246 | drain(50) →（vid≠0FCE）D8 + sleep50 + drain(50) → 1C 05 → awaiting_handshake=true → RX←2 必须 [1C,05] → 若 type2 则 authenticate_step("base") |
| 进入 uimlk | enter_uimlk 1249-1263 | enter_base → E7 → RX←1 必须 E7 → **无条件** authenticate_step("uimlk") |
| 认证 | authenticate_step 1164-1216 | 52 → RX←17；uimlk 允许前置 E7 ack **最多 4 次**（1171-1194，每次丢 1 B 再补 1 B）；校验 resp[0]==0x52 → decode_nibbles 得 8 B challenge → 发 0x53 + 16 nibbles → RX←2 必须 [53,01] |
| 退出 | finish_uimlk 1295-1312 / exit_base 1266 / exit_mode 1273 | uimlk：56→RX←1 必须 56，失败也记 exit_result（不抛错）；base：D8 + 50 ms；0FCE 设备**不发送** D8 |

**【事实】握手期 0xAF 剥离**：awaiting_handshake 为真时，recv 会持续剥掉缓冲头的 0xAF 并记为 async_events: pre_echo_delimiter（695-703）。这是唯一「协议层解析异步事件」的地方，必须在 core 保留（输出事件列表而非 JSON）。

**【事实】重试只有一处**：read_lockcode_inner 首次失败后 enter_base 再试一次（1534-1541），reset_lockcode_inner 同理（1574-1581）。其余操作**无重试**，一次超时即整条失败。

---

## 3. 时间与阻塞依赖：能否在 no_std 下 1:1 替换

### 3.1 全量清点（lib.rs）

| 用法 | 位置 | 数量 | no_std 替换 |
|---|---|---|---|
| std::time::{Duration, Instant} | 17 | 1 处 import | Duration 在 core 里同名同义（core::time::Duration），**不用改** |
| Instant::now() + saturating_duration_since | 679/683、740/744、1050/1053/1054、1167/1178 | 4 个 deadline 站点 | 换 now_ms() -> u64（trait 方法）或 embassy_time::Instant；比较与减法都是 u64 |
| thread::sleep(50ms) | 1223、1268 | 2 | 换 async fn delay(Duration)（协议级静默，不是退避） |
| 分片超时常量 | 500 ms(690)、50 ms(724)、1–200 ms(1055)、2000 ms(663)、3000 ms(27) | 5 个 | 全部原样保留；只是「每次 read 的 timeout 参数」 |
| f64 时间 | 1050 from_secs_f64、1055 as_secs_f64、432/1048/1079/2345 | 7 | **建议改 u32 ms**，见 R1 |

**【事实】** 没有 SystemTime、没有 chrono、没有定时器回调、没有线程/异步运行时依赖——协议是「同步顺序 + 两处主动延时」。移植不需要把阻塞改造成事件驱动，只需要把 4 个时钟点与 2 个 sleep 换成 trait 方法，**函数体可以逐行照抄**。

### 3.2 3 s 超时对 UI 的影响（交互设计的第一约束）

- **【事实】** 3 s 是**每个 recv 的 deadline**，不是整条操作的预算；status 会串行经历 ≥6 次 recv（§2.2）。
- **【推断】** 手机无响应时：readuimlk 20 s、clearlk 26 s、status 35 s（--type2 41 s）后才会报错；期间**没有任何进度回调**（log_sink 只在 verbose 下吐 TX/RX，603-611）。
- 桌面 GUI 不受影响：USB 全在后台线程（worker.rs:107-110，8 MB 栈），UI 每 120 ms 重绘（worker.rs:218-220），但**界面同样无进度**（README.zh-CN.md:30 只保证「不卡住」）。
- **对 OLED + 按键的含义**：必须（a）把单次传输超时从 3 s 降到 **500 ms–1 s**；（b）给每条操作加**总预算**（建议 5 s，超时后按键可取消）；（c）至少显示「等待手机… / 重试 n / 失败原因」。具体交互见 07 文档，本节只给约束。

---

## 4. std / alloc 依赖审计

### 4.1 逐项清单（次数为 lib.rs 全文计数，含测试）

| 依赖 | 次数 / 代表行 | 用途 | 性质 | no_std 方案 |
|---|---|---|---|---|
| std::fmt | 1（lib.rs:15 use）/ 66-75 | Display | 协议 | core::fmt，零改动 |
| std::error::Error | 77 | 错误 trait | 外壳 | 换 core::error::Error（Rust ≥1.81，10 号 spike 已验证） |
| std::sync::Arc | 1（16）/ LogSink 374 | 日志回调 | 外壳 | 改 trait LogSink { fn log(&mut self, &str) }（固件持有一个实现） |
| std::time | 1（17）| 见 §3 | 协议 | core::time::Duration + now_ms |
| std::thread::sleep | 1223、1268 | 协议静默 | 协议 | delay().await |
| std::fs | 10 / 1899-1935、3063-3096 | Linux sysfs、测试 | 外壳 | 丢弃 |
| std::path | 2 / 1899、2154 | 同上 | 外壳 | 丢弃 |
| std::process | 11 / 1972、2057 | reg.exe / PowerShell / driver.rs | 外壳 | 丢弃 |
| std::env | 1（测试 3064）+ driver.rs:76 | temp dir / 状态目录 | 外壳 | 丢弃 |
| std::collections::HashMap | 2 / 1961、2109；std::mem 1 / 1049 | Windows 设备名缓存 | 外壳 | 丢弃 |
| format! | 71 | 错误文案 + JSON 拼装 | **混合** | 见 R2 |
| to_string() | 41 | 同上 | **混合** | 见 R2 |
| String / Vec | 113 / 23 | 缓冲、结果、文案 | 混合 | 协议只需 1 个定长 rx 缓冲 |
| json! / Value | 63 / 193 | 结果表达 | 外壳 | 结构体 + to_json 适配 |
| hex:: | 15 | SN 解析、输出格式化 | 混合 | 保留（hex 0.4.3 支持 no_std，见 §5.4）或自写 40 行 |
| clap | 2 derive（458、488）+ Cli 2302 | 参数解析 | 外壳 | 丢弃 |
| rusb | 1 处 use（21）+ 50 余处调用 | 传输 | 外壳 | 换 UsbTransport 实现 |
| md-5 | 20、249 | **MD5** | **协议必需** | default-features=false（已核实 no_std） |
| println! / eprintln! | 51 / 11（62 行）| 人读报告、verbose | 外壳 | 丢弃（固件自己渲染） |

### 4.2 结论：协议真正必需的依赖只有 3 个

core::fmt、core::time::Duration、md-5（外加一个日志 trait 和一个时钟方法）。hex 只用于 SN/显示字符串，可留可替；其余全部是外壳。

### 4.3 缓冲与内存估算（【推断】，从常量与调用链推）

| 项 | 估算 | 依据 |
|---|---|---|
| rx 缓冲上界 | **≈768 B** | 单次 read 最多塞 512 B（[u8;512]，680/722/1051）+ 未消费的 expected_len ≤256（read_until limit） |
| 建议固定缓冲 | RxBuffer<1024> | 留 33% 余量，覆盖 probe 的 512 B 快照 |
| 栈峰值 | **≈1.5 KB** | recv 的 512 B + read_probe_response 的 512 B + 调用帧 |
| 结果结构体 | **<128 B** | LockState{control:u8,value:[u8;4]}、StatusReport 固定字段 |
| 桌面 JSON 峰值 | 1–2 KB/结果 | 一棵 serde_json Value 树（status 有 6 个字段 + 嵌套） |
| 结论 | core 可做到 **no_std + 无 alloc**：.bss≈1.3 KB、栈≤2 KB | RP2354 的 520 KB SRAM 完全不是瓶颈（平台细节见 09） |

---

## 5. 加密与鉴权：Type2Auth 的完整数学

### 5.1 计算链（全部可搬，无 I/O）

| 步骤 | 函数（行号） | 数学 |
|---|---|---|
| 1. SN → 8 B 材料 | parse_sn_to_type2auth 170-192 | 16 个 hex 字符：直接 hex::decode 用；否则前 min(5,len) 字符 ASCII 大写 + 其余 hex，得到字节串后**整体 reverse** |
| 2. 材料 → 密钥 K | derive_type2auth_key 228-246 | 先 reverse 得 s[0..8]，K = [bitrev8(s2), s5, s7, bitrev8(s3), s0, bitrev8(s6), bitrev8(s4), s1] |
| 3. K'（仅 4000） | 284-286 | 同样先 reverse 得 s，但置换不同：K' = [s0,s5,s2,s7,s4,s3,bitrev8(s6),s1]（与 K 的差别在位反转位置与字节顺序） |
| 4. 证明 | 277-290 | 4000：md5(K' 与 challenge 拼接)[0..8]；base/uimlk/2000：md5(K 与 challenge 拼接)[8..16] |
| 5. 组包 | 292-295 | 0x53 + encode_nibbles(8 B proof) = **17 B** |
| 6. 位反转 | bitrev8 223-225 | u8::reverse_bits()，core 原生 |

**【事实】** 服务名只允许 "base" | "uimlk" | "2000" | "4000"（271-275）；区分只在步骤 3/4。挑战由**手机**下发（8 B），设备端只做应答 → **无随机数、无时间戳、无防重放需求**。

### 5.2 现成测试向量（可直接做 no_std 回归，lib.rs:2964-3020）

| 用例 | 输入 | 期望输出 |
|---|---|---|
| 11 字符 SN → 材料 | SKYKA077394 | 947307414b594b53（2964-2968） |
| 16 hex SN | 947307414B594B53 | 原样 947307414b594b53（2971-2975） |
| 密钥派生 | 上面的材料 | 9a0794d253ce824b（2987-2994） |
| 认证包 base/uimlk | challenge 01 23 45 67 89 ab cd ef | 53050602070601030d0b0c000d0b070f01（3001-3008） |
| 认证包 4000 | 同上 | 5304030500090706010e0e0f01070b0d05（3009-3012） |

### 5.3 是否需要 DES/AES / RNG / 时间

**都不需要**【事实】：代码里除 md5 外无任何密码学 crate，无 rand，无时钟参与（Cargo.toml:27-33 只有 md-5 一个密码学依赖）。

### 5.4 依赖的 no_std 事实（本机 cargo registry 源码核对）

| crate | 版本 | 证据 | 结论 |
|---|---|---|---|
| md-5 | 0.10.6 | src/lib.rs:26 = #![no_std]；Cargo.toml:51 default=["std"] | default-features=false 即可，软实现 |
| digest | 0.10.7 | src/lib.rs:25 = #![no_std] | 传递依赖同样安全 |
| hex | 0.4.3 | src/lib.rs:36 cfg_attr(not(feature="std"), no_std) | 需要 alloc 才能 hex::encode，否则用 encode_to_slice |
| rusb | 0.9.4 | 依赖 libc + libusb1-sys（C 库）+ build.rs | **不可 no_std**，只能留在桌面 |
| serde_json | 1.0.154 | Cargo.toml:62-64 有 alloc feature，默认 std | 可 no_std+alloc，但**建议不进 core** |

**【推断】** 性能：MD5 只对 16 B 消息算 1 次（1 个 block），Cortex-M33 @150 MHz 软实现 <10 µs，与 USB 传输（ms 级）相比可忽略。RP2350 无 MD5 硬件加速（仅 SHA-256，详见 09），无需考虑加速。

---

## 6. 移植边界建议（重点）

### 6.1 三条原则

1. **core 不认识 USB**：只认识「写一段字节 / 读一段字节 / 等一会儿 / 现在几点」。端点、接口、claim、VBUS 全是固件的事。
2. **逻辑照抄，接口重画**：1159-1815 的 530 行操作序列逐行搬进 core 的 ops.rs，只把 self.send / self.recv / self.drain 换成 trait 调用。
3. **桌面输出零变化**：run_task(&DeviceConfig,&Task) -> AuRokkuResult<Value>（2179）签名不动，内部变成「core 跑完 → Report::to_json」，GUI 的 36 处字段访问（app.rs）不受影响。

### 6.2 目录树（建议）

```text
auroku-rs/
├─ Cargo.toml                    # workspace: auroku, auroku-core, auroku-driver-sys
├─ crates/auroku-core/           # 新增：no_std，不含任何宿主依赖
│  ├─ Cargo.toml                 #   md-5(default-features=false); features: std/alloc/serde
│  └─ src/
│     ├─ lib.rs                  #   re-export；forbid(unsafe_code)
│     ├─ error.rs                #   ErrorKind/Error/Detail/HexBuf<N>
│     ├─ codec.rs                #   encode/decode_nibbles, lockcode digits, hex_spaced
│     ├─ type2auth.rs            #   parse_sn, bitrev8, derive_key, compute_request, md5
│     ├─ task.rs                 #   Task/ProbeKind/BatchProfile（去 clap derive）
│     ├─ report.rs               #   LockState/StatusReport/Mutation/Iccid/Pin...
│     ├─ transport.rs            #   UsbTransport + LogSink + 时钟
│     ├─ rx.rs                   #   RxBuffer<const N>（recv/read_until/drain 语义）
│     ├─ session.rs              #   enter_base/enter_uimlk/authenticate_step/exit_*
│     └─ ops.rs                  #   read_state/mutate/iccid/pin/maint/status/batch/probe
├─ src/                          # 桌面 crate（保持包名 auroku）
│  ├─ lib.rs                     #   薄壳：rusb 实现 + clap + JSON + driver 转发
│  ├─ transport_rusb.rs          #   新增：impl UsbTransport for RusbTransport
│  └─ driver.rs                  #   原样保留（Windows）
└─ firmware/（另一仓库或 fw/）    # RP2354 侧：embassy + USB host + OLED + 按键
```

### 6.3 UsbTransport trait 草案

```rust
// crates/auroku-core/src/transport.rs
use core::time::Duration;

pub trait UsbTransport {
    type Error: core::fmt::Debug;

    /// 已配置的 OUT 管道写（Bulk/Interrupt 由固件按描述符决定）。返回写入字节数。
    async fn write(&mut self, data: &[u8]) -> Result<usize, Self::Error>;

    /// 已配置的 IN 管道读；timeout 内无数据返回 Err。
    async fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, Self::Error>;

    /// 协议级静默延时：仅 0xD8 之后 50 ms（lib.rs:1223/1268），不是退避。
    async fn delay(&mut self, d: Duration);

    /// 单调毫秒时钟，替代 Instant::now()（lib.rs:679/740/1050/1167）。
    fn now_ms(&self) -> u64;
}

pub trait LogSink { fn log(&mut self, line: &str); }   // 替代 Arc<dyn Fn>（lib.rs:374）
```

**固件侧职责（core 不做）**：USB host 控制器初始化、VBUS/端口供电、attach/detach 与重枚举、SET_CONFIGURATION、按 VID/PID + iSerialNumber 匹配设备、find_interface 的等价实现（331-367）、claim / set_alternate_setting（925-930）、把 0x04/0x84 解析成实际端点、维护 per-transfer 超时。

### 6.3b sync trait 变体与选择建议（与 06 的架构决策对齐）

**背景【待决策】**：06 总报告可能选择「Rust core 编成 **staticlib + C ABI**，供 Pico SDK / TinyUSB 的 C/C++ 固件调用」。这条路线下 async trait 会把 Rust future/executor 拖到 FFI 边界，反而增加复杂度；因此 core 的传输 trait 必须能是 **同步** 的。

| 维度 | async 版（§6.3） | sync 版（本节） |
|---|---|---|
| 与 lib.rs 的对应 | 每个 .await 点 ≈ 原阻塞调用点 | **逐行 1:1**：read(timeout) ≡ read_endpoint_in(647-653)；delay ≡ thread::sleep(1223/1268)；now_ms ≡ Instant::now(679) |
| 状态机 / 操作序列 | 同一份 | **同一份**——只改 trait 边界，ops/session 的报文序列与校验一个字不动 |
| 目标固件形态 | 纯 Rust：embassy 任务、真异步 USB host | C TinyUSB（tuh_task 轮询式）或裸机主循环；staticlib + C ABI |
| FFI 友好度 | 差：future/Waker/executor 不能跨 C ABI | 好：C 侧只需一个同步入口 + 出参缓冲（见下） |
| 超时归属 | 传输层 async 超时 | 传输层同步超时；C 侧用 tuh_task 轮询 + 时间戳 deadline 实现 |
| 桌面复用 | 需要 block_on 垫片 | 零垫片（rusb 本来就是阻塞调用） |
| 代价 | 多一个 executor 依赖 | 长操作会阻塞调用线程（对单任务固件无影响） |

```rust
// crates/auroku-core/src/transport.rs（sync 变体；与 6.3 只差 async/await 两个词）
use core::time::Duration;

pub trait UsbTransport {
    type Error: core::fmt::Debug;

    /// 阻塞式 OUT 写，返回写入字节数。        ≡ lib.rs:655 send()
    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<usize, Self::Error>;
    /// 阻塞式 IN 读，timeout 内无数据返回 Err。 ≡ lib.rs:690 read_endpoint_in()
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, Self::Error>;
    /// 协议级静默延时（仅 0xD8 后 50 ms）。    ≡ lib.rs:1223/1268 thread::sleep
    fn delay(&mut self, d: Duration);
    /// 单调毫秒时钟，替代 Instant::now()。      ≡ lib.rs:679/740/1050/1167
    fn now_ms(&self) -> u64;
}

// staticlib 导出给 C 的同步入口（示意；出参为固定缓冲，避免跨 FFI 的分配/panic）
// #[no_mangle] pub extern "C" fn auroku_run(op: u8, sn: *const u8, out: *mut u8, cap: usize) -> i32;
```

**选择规则（建议写进 06）**：
1. 固件走 **C/C++ + TinyUSB 或裸机轮询 → 选 sync**；core 只暴露一个同步 C 入口，OS 侧（桌面）直接调用同一套函数，不需要 block_on。
2. 固件走 **纯 Rust embassy → 选 async**；此时不要为了 FFI 再做一个 sync 版。
3. **不要同时维护两套**：ops/session 只写一份，用 feature 切换 trait 定义（或 maybe-async 宏）；两套 trait 定义本身的样板量约 40 行，可接受【待验证：maybe-async 在 no_std 下的可用性】。
4. 对 §8 的影响：若选 sync + C ABI，U7 从「embassy transport」变为「TinyUSB host 回调 + 一个 extern "C" 入口」，**工作量不变（5–10 人日）但少一个 embassy 依赖**；U2/U5 的 block_on 垫片也随之取消（-0.5 人日）。

### 6.4 引擎草案

```rust
pub struct Session<T> {
    bus: T,
    rx: RxBuffer<1024>,
    material: Option<[u8; 8]>,
    sn: Sn,           // [u8;16] + len，无 String
    vid: u16,
    type2: bool,
    events: EventLog, // 替代 async_events: Vec<Value>
}

impl<T: UsbTransport> Session<T> {
    pub async fn connect(&mut self, cfg: SessionConfig<'_>) -> Result<(), Error>;    // 无 USB 动作，仅解析 SN
    pub async fn enter_base(&mut self) -> Result<(), Error>;                         // lib.rs:1219
    pub async fn enter_uimlk(&mut self) -> Result<(), Error>;                        // lib.rs:1249
    pub async fn authenticate_step(&mut self, s: Service) -> Result<(), Error>;      // lib.rs:1164
    pub async fn read_state(&mut self) -> Result<LockState, Error>;                  // lib.rs:1319
    pub async fn run_task(&mut self, task: Task) -> Result<Report, Error>;           // lib.rs:2220 dispatch
}

pub async fn run<T: UsbTransport>(bus: T, cfg: SessionConfig<'_>, task: Task)
    -> Result<Report, Error>;   // 对应 lib.rs:2179 run_task
```

### 6.5 错误模型（替代 71 个 format!）

```rust
pub enum ErrorKind { Value, Runtime, Timeout, Driver, Bus }  // 前 3 个保持 lib.rs:56-63 的字符串
pub enum Detail { None, Hex(HexBuf<24>), Count(u32), Pair(u8, u8) }
pub struct Error { pub kind: ErrorKind, pub code: &'static str, pub detail: Detail }
```

桌面侧用 code → 原文案 映射表恢复现有英文消息（如 1198 的 "Invalid challenge frame for {service}: {hex}"），保证 CLI/JSON 输出与 v0.3.1 可对比；固件侧只用 code + 本地化短句。**代价**：文案不是字节级一致，需要一条 golden 对比测试兜底（U4）。

### 6.6 Report 与 JSON 适配

| core 类型 | 承载字段（与原 JSON 键完全一致） |
|---|---|
| LockState | control、lock_value、binding_cleared、raw_hex |
| StatusReport | operation / sn / version / lockcode / uim_lock{control,lock_value,unlocked} / iccid_record / success |
| Mutation | operation / before / after / reply / command_accepted / verified / partial_write_possible |
| Iccid | raw_hex、decoded_hex |
| Pin / ResetPin | lockcode、verified |
| Maintenance | maintenance_bit、command_accepted |
| Probe | probe / selection / request_hex / response_hex / responsive / matched（usb 子树由固件提供或省略） |

**关键约束【事实】**：GUI 有 36 处按键名取字段（app.rs:396-2331），其中 uim_lock.unlocked(1371)、verified(1439/1686/1775)、command_accepted(1449)、before/after(1680-1683)、exit.acknowledged(2295)、async_events(2331) 都是强契约。core 结构体字段名必须逐字对齐，否则 GUI 静默显示空白。

### 6.7 feature 方案（与桌面代码共享）

| crate | feature | 默认 | 作用 |
|---|---|---|---|
| auroku-core | （无） | 是 | no_std、无 alloc：定长 RxBuffer + &'static str 错误码 |
| | alloc | | 允许 String/Vec 缓冲与携带 hex 文案 |
| | std | | now_ms 的 Instant 参考实现 + 桌面便利；错误 trait 用 core::error::Error，不再需要 std |
| | serde | | Report 派生 Serialize，桌面直接 to_value 出 JSON |
| auroku（桌面） | 既有 gui / static-libwdi 不变 | | 依赖 core 时开 std,alloc,serde |

**桌面改造点（3 处）**：① run_task 内部变为 block_on(core::run(RusbTransport::new(cfg), ...))（新增 pollster 或 futures 依赖）；② RusbTransport 把 recv 的 500 ms 分片与 deadline 逻辑上移（或直接复用 core 的 RxBuffer）；③ print_human_report 改为读 Report 而不是 Value（或先 to_json 再走老代码，最省事）。

---

## 7. 隐藏坑与风险清单

| # | 风险 | 证据（行号） | 影响 | 处置 |
|---|---|---|---|---|
| R1 | **f64 超时**：Task::Probe{timeout_secs: f64} 与 from_secs_f64 | 432、1048、1050、1055、2345 | M33 为单精度 FPU（见 09），f64 走软浮点；引入 compiler-builtins 浮点路径与代码膨胀 | 改 timeout_ms: u32；CLI 侧解析成 ms |
| R2 | **格式化依赖**：71 个 format! 中约 20 个在协议路径（747/1198/1234/1255/1286/1324/1446/1487/1583 等） | 同上 | no_std 无 alloc 时无法拼 hex 错误串 | Detail::Hex(HexBuf<N>) + core::fmt::Write 到固定缓冲 |
| R3 | panic / unwrap | 非测试仅 2 处 expect（812/818），0 个 unwrap | 实际风险**低**；但 MCU panic=挂死 | 固件设 panic=abort + 记录复位原因；core 保持 #![forbid(unsafe_code)]（lib.rs:1） |
| R4 | **日志 sink**：Arc<dyn Fn(&str)> + eprintln! | 374、603-631 | no_std 无 dyn 闭包、无 stderr | LogSink trait；固件实现为环形缓冲 → OLED/串口 |
| R5 | **错误文案 / locale**：英文协议错误 + 中文 GUI（DeviceSummary::display_name 1885 硬编码「USB 设备」） | 85-104、1885、worker.rs:19-33 | 固件直接显示长英文串会溢出 128×64 屏 | 错误码枚举 + UI 层本地化（与 i18n.rs 同思路） |
| R6 | **Windows 专有逻辑** | 85-104、1964-2090、driver.rs 全 719 行 | 与 MCU 无关 | 整体不进 core |
| R7 | **设备拔出/重连**：桌面每 op 新建 session 后 close（2179-2198），无重连；drain 吞错（729） | 同上 | MCU 上拔线是常态，必须 attach/detach 状态机；吞错会掩盖 stall 让后续交互错位 | core 的 drain 返回 Result；固件层做「拔线→中止当前 op→回首页」 |
| R8 | **端点差异**：0x04/0x84 是默认，probe 只枚举 bulk 对 | 29-31、996-1019 | 若真机是 Interrupt 端点，probe list 页看不到候选；固件若写死 bulk 会失败 | 固件按自己枚举出的 transfer_type 建 XferKind，等价 316-328 |
| R9 | **控制传输的 wIndex/接口号** | 全仓无 read_control / write_control | 任务书假设不成立；只有枚举期标准请求（SET_CONFIGURATION wIndex=0、SET_INTERFACE wIndex=interface） | 由固件 USB 栈负责，core 不需要任何 request_type 参数 |
| R10 | **手机侧对 host 的依赖** | 无 CDC 类请求（全仓无 control 传输） | 好消息：**不需要 CDC 类驱动**；且平台能力已核实（RP2350 USB 1.1 host、TinyUSB 0.17+ 支持 RP2350 host、pico-sdk 2.3.1 pin TinyUSB 0.18/0.21 修 hard fault）。仍待实测：VBUS 5 V/500 mA、复位时序、上电到可交互的延时 | 09/10 文档给实测数据；core 侧只需 delay 与「重试 n 次」策略 |
| R11 | **批量/状态耗时** | §2.2、§3.2 | status 最坏 35 s；批量查询与单条查询几乎同价（都在一次 session 内） | UI 给总预算 + 可取消；固件优先暴露 status（一条命令拿全量） |
| R12 | **无重试机制** | 仅 1534/1574 两处 fallback | USB 传输在 MCU 上错误率高于桌面（线缆/供电） | core 增加 retry: u8 策略，只对 Timeout 重试 |
| R13 | **JSON 兼容契约** | app.rs 36 处字段访问；preserve_order（Cargo.toml:33） | 字段改名/嵌套变化会让 GUI 静默空白 | 结构体字段 = 原键名；golden JSON 对比测试 |
| R14 | **SN 依赖过宽**：connect(true) 让 readpin / exit 也要 SN | 2210、765-865 | MVP 若照抄，就必须先能读 iSerialNumber | core 按操作声明 require_sn：只有 uimlk/auth 相关操作需要 |

---

## 8. 工作量估算

**【推断】** 以「熟悉本仓库的 1 名工程师 = 1 人日（6 h）」计；不含 OLED/按键 UI 与固件应用逻辑（07/10 文档）。

| # | 拆分单元 | 主要工作 | 新增/改动 LOC | 人日 |
|---|---|---|---|---|
| U1 | 协议编解码 + 鉴权 | codec.rs/type2auth.rs，含 19 个单测里 8 个纯向量的迁移 | 移 156 + 测试 120 | 1.0 |
| U2 | 传输抽象 + RxBuffer + 错误模型 | transport.rs/rx.rs/error.rs；把 80 余处 send/recv/drain 调用点改成 trait | 新 350 + 改 200 | 2.5 |
| U3 | 操作/会话状态机 | session.rs/ops.rs：1159-1815 共 530 行 async 化 | 移 530 + 胶水 80 | 3.0 |
| U4 | Report 结构体 + JSON 适配 + 兼容回归 | report.rs + 桌面 to_json；对齐 36 处 GUI 字段 | 新 250 | 2.0 |
| U5 | 桌面 rusb 适配 | transport_rusb.rs + run_task 包 block_on；CLI/GUI 行为不变 | 新 180 改 60 | 1.5 |
| U6 | 测试迁移 + mock 回放 | 19 单测迁移；用伪 transport 回放 §2.2 的报文序列 | 新 200 | 1.5 |
| **core 小计** | | | **≈1900 行** | **11.5** |
| U7 | 固件 UsbTransport 实现 | USB host 栈对接、枚举、端点发现、热插拔、VBUS | 新 400–800 | **5–10** |
| U8 | 固件会话/结果集成 | 与 OLED/按键状态机对接、错误码本地化 | 新 250 | 2–3 |
| U9 | 共享回放夹具（可选） | 桌面抓真实 TX/RX 存为 fixture，固件离线跑 | 新 150 | 1.0 |

**MVP（只读）范围**：probe seri/scdp + readpin + exit + 可选 status（需 SN）。对应 U1+U2+U5 子集 + U6 + U7 + U8 ≈ **12–18 人日**（不含 core 的写操作与 batch）。
**完整版**：12 个 Task 全量 + status/batch + 超时/重试/热插拔 + JSON 兼容 ≈ **20–28 人日**（core 11.5 + 固件 7–12 + 集成 2–3 + 缓冲 2）。

**建议顺序**：U1 → U2 → U6（**先在桌面用 mock 跑通，不碰硬件**）→ U5 → U3 → U4；U7 由固件侧并行启动（它才是关键路径）。core 的每一步都能在桌面上用 cargo test 验证，风险可控。

---

## 9. 结论

1. **可移植比例**：lib.rs 代码行 2574 中 **≈900 行（36%）**是真正可搬的协议内核；**≈1390 行（54%）**是 rusb/CLI/JSON/宿主绑定；其余为测试与注释。按功能看，**12 个 Task 变体、15 条报文序列、4 种服务认证全部可移植**，唯二不可移植的是「描述符枚举（probe list）」与「Windows 驱动安装（install/restore-driver）」。
2. **推荐切法**：新建 crates/auroku-core（no_std，5 个模块 + 1 个 trait；trait 边界按 §6.3b 选 async 或 sync），桌面保留 AuRokkuDevice 的位置改为 rusb 对 trait 的实现；run_task 签名不动，JSON 由 Report::to_json 生成（键名逐字保留）。**横切面在 lib.rs 第 1159 行之前**。
3. **最大技术风险不在协议层**：协议是「发 1–17 字节、收 1–21 字节」的同步问答，没有加密难点、没有时序竞争、没有 no_std 阻塞（§3/§5 已逐条核实）。真正的风险是 ① 固件 USB host 栈能否稳定枚举这块 CDC 复合设备并完成热插拔（R7/R8/R10）；② 3 s 级超时叠加后 UI 体验（R11）；③ JSON 兼容契约（R13）。
4. **不建议**做的事：不要为 MCU 重写「事件驱动状态机」——现有顺序式代码在 embassy 里用 async fn 逐行照抄即可，重写状态机会把 530 行协议逻辑变成 1500 行不可验证的分支表。
5. **建议的门禁**：core 抽取完成后，桌面端 cargo test + 一次真机 --op status --json 输出与 v0.3.1 逐字节对比，作为进入固件阶段的准入条件。

---

## 附录 A · 本文断言的代码位置索引

| 断言 | 位置 |
|---|---|
| 无 control transfer / 只有 4 种传输调用 | lib.rs:650-651、665-666（全仓 grep read_control / write_control / bmRequestType 无命中） |
| 端点默认值与方向校验 | lib.rs:29-31、2332-2337、2837-2844 |
| 端点发现与传输类型判定 | lib.rs:316-328、331-367、921-930 |
| 5 个底层原语 | lib.rs:655-676、678-713、716-732、734-758、1048-1076 |
| 会话状态机 | lib.rs:1164-1216、1219-1246、1249-1263、1266-1312 |
| 12 个操作入口 | lib.rs:1319、1341、1443、1470、1475、1520、1560、1612、1680、1730、1797、1079 |
| Type2Auth 数学 | lib.rs:170-192、223-246、248-257、260-296 |
| 测试向量 | lib.rs:2964-3020 |
| 无 unwrap（非测试）/ 2 处 expect | lib.rs:812、818（全文件仅此 2 处） |
| 36 处 JSON 字段访问 | src/bin/auroku-gui/app.rs:396-2331 |
| 桌面后台线程与重绘 | src/bin/auroku-gui/worker.rs:107-110、218-220 |
| Windows 驱动栈（整体丢弃） | src/driver.rs:1-719 |
| 依赖清单 | Cargo.toml:27-33 |
| md-5 / hex / digest 的 no_std 证据 | 本机 registry：md-5-0.10.6/src/lib.rs:26、hex-0.4.3/src/lib.rs:36、digest-0.10.7/src/lib.rs:25 |
| rusb 依赖 C 库 | 本机 registry：rusb-0.9.4/Cargo.toml:33-37 |

## 附录 B · 未决问题（需 lead / platform-researcher / ui-designer 回填）

1. **【待验证】** 目标手机的 SERI 接口实际是 Bulk 还是 Interrupt？端点号是否恒为 4？→ 需要真机 --op probe --json 的 bulk_candidates 输出（若为空则说明是 Interrupt，而 probe 不列 interrupt 对）。
2. **【平台已核实】** RP2350 官方文档明确 USB 1.1 host+device（[RP2350 datasheet](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf)）；TinyUSB 0.17+ 支持 RP2350 host（[TinyUSB releases](https://github.com/hathach/tinyusb/releases)）；pico-sdk 2.3.1 pin TinyUSB 0.18、0.21 修了 RP2350 host 的 hard fault（[pico-sdk releases](https://github.com/raspberrypi/pico-sdk/releases)）。**仍需实测**：对 EP4 IN/OUT 双 pipe 的并发、外部 5 V VBUS 供电与过流保护 → 09 文档。
3. **【待验证】** 手机从「上电/插入」到「能应答 0x1C 0x05」需要多久（影响固件的 attach 后延时与重试次数）→ 需实测。
4. **【决策】** MVP 是否包含 status？它要求读 iSerialNumber 或手工输 SN；若排除，MVP 只剩 probe + readpin + exit（信息量偏低）。建议：MVP 含 SN 读取（固件枚举期已有），把 status 纳入。
5. **【决策】** 桌面端是否接受「错误文案由 code 映射、非字节级一致」（R2/R5）？若不可接受，core 需保留 alloc + 原文案模板（+1 人日）。
6. **【已闭环】** `undefined/` 已清理；`spike/` 随 rp2354 分支提交（`target/` 已忽略），作为 10 号文档的可复现证据。
