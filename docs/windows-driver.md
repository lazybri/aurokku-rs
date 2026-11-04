# Windows USB 驱动说明

本文说明 auRokku 在 Windows 上为什么需要 WinUSB 驱动、如何安装,以及为什么
“直接用 COM 口”这条路走不通。

## 1. 现象

设备(以 `04C5:1270` 为例)插入 Windows 后,设备管理器出现“USB 串行设备 (COMx)”,
属性事件显示“已添加驱动程序服务 (usbser) / 需要进一步安装”;此时:

- `auroku --op probe --vid 04C5 --pid 1270` 能列出描述符(纯枚举,不需要驱动);
- `auroku --op probe --vid 04C5 --pid 1270 --probe-kind seri` 报
  `USB error: Operation not supported`;
- 用串口助手或 .NET `SerialPort.Open("COM3")` 打不开该 COM 口,报 `0x8007001F`
  (设备未正常工作)。

## 2. 设备实际拓扑

| 接口 | 类/子类/协议 | 端点 | 用途 |
| --- | --- | --- | --- |
| 0 | `02/02/01` | 中断 `0x82` | CDC 通信接口 |
| 1 | `0A/00/00` | Bulk `0x03` / `0x83` | CDC 数据接口(被 usbser 用) |
| 2 | `02/0A/01` | Bulk `0x04` / `0x84` | SERI 协议口(auRokku 实际使用) |

设备级 `bDeviceClass = 0x02`,所以 Windows 的兼容 ID 是
`USB\Class_02&SubClass_02&Prot_01`,由 usbser.sys **一个函数驱动占住整机**,
不会生成 `&MI_00/&MI_01/&MI_02` 子节点。

由此得到两个结论:

1. 接口 2(SERI)没有任何 inbox 驱动,Windows 无法对它单独绑定 WinUSB;
2. 装 WinUSB 只能针对整机,装完 COM 口必然消失——但它本来也打不开,没有损失。

## 3. 安装 WinUSB

### 方式 A:程序内一键安装(发布版内置 libwdi,推荐)

发布版的 `auroku.exe` / `auroku-gui.exe` 在 CI 中**静态链接了 libwdi**,所以:

- GUI:概览页**常驻「WinUSB 驱动」管理卡**(Windows 上始终显示),显示目标设备、
  当前 Service(`WINUSB` / `usbser` / 其他)、**安装 WinUSB 驱动** / **恢复原驱动** /
  **刷新驱动状态** 按钮、驱动日志路径与复制按钮,以及「退出时自动恢复原驱动」
  开关(默认关闭);无需先触发一次驱动错误;
- 点击后只弹**一次**标准 UAC 对话框(标题是本程序),没有控制台窗口、没有额外 exe;
- 安装成功后自动重试刚才失败的操作;
- 恢复时用 `pnputil /remove-device` 移除设备节点,Windows 重新枚举后自动绑回 usbser。

UAC 的处理刻意做成对小白最省事的形式:

1. 先在本进程内调用 `wdi_install_driver`;**只有** libwdi 返回
   `WDI_ERROR_NEEDS_ADMIN` 才提权——如果用户本来就在管理员终端里,完全不会弹窗;
2. 提权通过 `Start-Process -Verb RunAs` 启动**本程序自己**
   (`auroku.exe --op install-driver --vid ... --pid ...`),不是第三方工具,因此
   UAC 上显示的是 auRokku;
3. 取消 UAC 只会得到一条“elevated helper exited with code ...”错误,不会留下半装状态。

每一步都会写入 `%LOCALAPPDATA%\auroku\driver.log`(目录可用 `AUROKU_STATE_DIR`
覆盖):**每次用户操作截断重写**,提权子进程只追加。提权助手以隐藏方式运行,
不会再弹出控制台 / PowerShell 窗口。

CLI(与 GUI 按钮完全等价):

```bat
auroku --op install-driver --vid 04C5 --pid 1270
auroku --op install-driver --vid 04C5 --pid 1270 --restore-on-exit
auroku --op restore-driver --vid 04C5 --pid 1270
```

源码构建且没有 libwdi 时(未启用 `static-libwdi` 特性),程序会回退到外部
`wdi-simple.exe`:查找顺序为 `AUROKU_WDI_SIMPLE` → 程序目录 → 程序目录下的
`tools` → 当前目录下的 `tools` → `PATH`;找不到时明确报错并说明放置位置。

CLI 的 `--restore-on-exit` 不会让一次性进程立刻卸载,它只把“退出时恢复”的意图
写进状态文件,由之后退出的 GUI 或手动 `--op restore-driver` 完成。

安装结果记录在 `%LOCALAPPDATA%\auroku\driver-state.json`(可用
`AUROKU_STATE_DIR` 覆盖)。如果勾了「退出时恢复」但进程被强杀,下次启动时概览页的
「WinUSB 驱动」卡会显示待恢复警告,可直接点「恢复原驱动」完成兜底。

### 方式 B:Zadig(手动,最快)

1. 以管理员身份运行 Zadig;
2. `Options` -> 勾选 `List All Devices`;
3. 下拉框里只有**整机**条目(如 `USB 串行设备 (COM3)`),不要期待出现
   `Interface x`;
4. 目标驱动选 `WinUSB`,点 `Replace Driver`;
5. 成功后设备管理器里设备名变为 `WinUSB device`,COM 口消失,这是预期行为。

### 方式 C:wdi-simple(手工执行,便于排查)

`wdi-simple.exe` 是 libwdi 的命令行工具(Zadig 的同一套内核),程序内的一键安装
就是调用它。手工执行便于排查:

```bat
wdi-simple.exe --vid 0x04c5 --pid 0x1270 --type 0 --name "auRokku USB"
:: --type 0 = WinUSB;参数以 wdi-simple --help 为准
:: 卸载:wdi-simple.exe --vid 0x04c5 --pid 0x1270 --type 0 --uninstall
```

安装需要一次管理员提权(UAC),会生成并信任一张自签名证书——与 Zadig 的行为
一致。

### 方式 D:签名 INF(正式发布)

INF 必须匹配**整机**硬件 ID(不能带 `&MI_`),并写入 `DeviceInterfaceGUIDs`,
否则 WinUSB 不会创建设备接口,libusb 依然找不到设备:

```ini
[Version]
Signature="$Windows NT$"
Class=USBDevice
ClassGuid={88BAE032-5A81-49f0-BC3D-A4FF138216D6}
Provider=%MFG%
CatalogFile=auroku.cat
DriverVer=11/04/2026,1.0.0.0

[Manufacturer]
%MFG%=Devices,NTamd64,NTarm64

[Devices.NTamd64]
%DEVNAME%=WINUSB_Install, USB\VID_04C5&PID_1270
[Devices.NTarm64]
%DEVNAME%=WINUSB_Install, USB\VID_04C5&PID_1270

[WINUSB_Install]
Include=winusb.inf
Needs=WINUSB.NT

[WINUSB_Install.Services]
Include=winusb.inf
Needs=WINUSB.NT.Services

[WINUSB_Install.HW]
AddReg=WINUSB_AddReg
[WINUSB_AddReg]
HKR,,DeviceInterfaceGUIDs,0x10000,"{6F1D2B60-D5A0-4A5B-9A3C-7C3B2E5D9A11}"
```

安装:

```bat
pnputil /add-driver auroku.inf /install
```

硬件 ID 匹配的优先级高于 usbser 使用的兼容 ID,因此可以顶掉 usbser。注意
Windows 10/11 x64 要求驱动包带有效签名(微软 attestation 或自签名证书导入
受信任根),否则 `pnputil` 会拒绝安装。

### 方式 E:临时禁用 usbser.sys(仅测试/排查)

如果只是想在测试机上阻止 Windows 把 CDC 设备绑定到 `usbser.sys`,可以临时
把该内核服务的 `Start` 设为 `4`(`SERVICE_DISABLED`):

- GUI:概览页 **WinUSB 驱动** 卡片底部的 **禁用 usbser.sys（测试）** /
  **恢复 usbser.sys** 按钮(确认后弹一次 UAC);
- `tools/windows/disable-usbser.reg` — 直接导入的注册表条目;
- `tools/windows/usbser-service.ps1` — 带原值备份/恢复、状态查询和
  `-WhatIf` 的脚本:

  ```powershell
  .\tools\windows\usbser-service.ps1 -Action Status
  .\tools\windows\usbser-service.ps1 -Action Disable   # 管理员权限
  .\tools\windows\usbser-service.ps1 -Action Restore   # 恢复原始 Start
  ```

禁用后需重启或重新插拔设备才生效;它会影响机器上所有 CDC 串口设备,且
**不会安装 WinUSB 或让 libusb 直接访问设备**。完整说明与恢复步骤见
[windows-disable-usbser.md](windows-disable-usbser.md)。

## 4. 验证

```bat
:: 1) 驱动是否换成 WinUSB(应输出 WINUSB 而不是 usbser)
reg query "HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_04C5&PID_1270" /s /v Service

:: 2) SERI 是否可通:期望回显 1c 05
auroku --op probe --vid 04C5 --pid 1270 --probe-kind seri --verbose

:: 3) 完整状态
auroku --op status --vid 04C5 --pid 1270
```

## 5. 恢复原来的 COM 口

设备管理器 -> 右键设备 -> 卸载设备 -> 勾选“删除驱动程序” -> 扫描硬件改动。
设备会重新以“USB 串行设备 (COMx)”出现(仍然打不开,属设备本身 CDC 功能的问题)。

## 6. 常见问题

- **列表里找不到设备?** 确认 `Options -> List All Devices` 已勾选,并换一个
  USB 口/线;部分 hub 会让设备重新枚举。
- **装完 WinUSB 后工具仍报 not found?** 确认 VID/PID 与设备一致,并确认没有
  同时运行占用设备的其他工具。
- **能否既保留 COM 口又用 WinUSB?** 该设备是单一 CDC 功能(设备级 class 02),
  不行;需要固件把 SERI 接口声明成带 IAD 的独立功能,或提供 WCID 描述符。

## 7. 构建(只在 CI)

Windows 发布包里的 libwdi 是静态库,由仓库 **orphan 分支 `libwdi-static`** 上的
可复用流水线构建;产品 CI 通过**不可变 tag** `libwdi-1.5.1-x64-r1` 引用它,
`.github/workflows/build.yml` 不再内联下载 WDK、clone 上游或调用 msbuild:

1. 产品 CI 的 `libwdi` job 用 `workflow_call` 调用
   `lazybri/aurokku-rs/.github/workflows/libwdi-static.yml@libwdi-1.5.1-x64-r1`,
   构建出的静态库经 SHA256、COFF 符号与静态 CRT 校验后,作为 `libwdi-static`
   artifact 传给 Windows 构建 job;
2. Windows job 把 artifact 解到 `libwdi-out` 并导出
   `LIBWDI_LIB_DIR=<artifact>/lib`;`.github/scripts/build-windows.ps1` 检测到该
   变量后自动加 `--features static-libwdi` 编译并打包,同时从
   `LIBWDI_LIB_DIR/COPYING-LGPL` 取 LGPL 许可文件放进 zip;
3. 升级 libwdi = 在 `libwdi-static` 分支发新 tag `libwdi-<版本>-x64-r<N>`,再改
   产品仓库里的引用;tag 是供应链锚点,发布后不可移动。

本地没有 WDK 也能正常 `cargo build`:特性默认关闭,驱动安装回退到外部
`wdi-simple.exe`。要在本地复现静态链接,先自行构建(或从 Release 下载)
`libwdi.lib`,再设 `LIBWDI_LIB_DIR` 指向同时含 `libwdi.lib` 与 `COPYING-LGPL`
的目录后,在 Windows 上执行 `cargo build --features static-libwdi`。

## 8. 许可证与合规

auRokku 整体以 **GPL-3.0-only** 发布,完整许可证见仓库根目录 `LICENSE`;
对应源码(Corresponding Source)是名字与发布版本相同的 Git tag,例如
<https://github.com/lazybri/aurokku-rs/tree/v0.3.6>。发布包内附
`LICENSE`、`THIRD-PARTY-NOTICES.md`、`LICENSES/LGPL-3.0.txt` 与
`LICENSES/LGPL-2.1.txt`。

- **libwdi(Windows 发布版,静态链接)**:LGPL-3.0。发布包附带
  `LICENSE-LGPL-3.0.txt` 与 `LICENSES/LGPL-3.0.txt`;上游源码固定在
  commit `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602`,构建流水线在
  `libwdi-static` 分支公开。若要使用修改过的 libwdi,按第 7 节把新的
  `libwdi.lib` 放进 `LIBWDI_LIB_DIR` 后重新构建即可完成重新链接。
- **libusb(rusb vendored,静态链接)**:LGPL-2.1,许可证文本见
  `LICENSES/LGPL-2.1.txt`,源码 <https://github.com/libusb/libusb>
  (libusb 1.0.27,由 `libusb1-sys` 0.7.0 提供)。
- **Qt 6(仅 `qt6` 分支的 GUI,动态链接)**:LGPL-3.0;替换匹配版本的 Qt
  DLL / dylib 即可重新链接,源码 <https://code.qt.io>。本分支(main)的 GUI
  使用 egui/eframe,不链接 Qt。
- 其余 Rust crates 使用 MIT / Apache-2.0 等宽松许可证,完整依赖清单见
  `Cargo.lock` 与 `THIRD-PARTY-NOTICES.md`。

所有 LGPL 组件均为未经修改的上游版本;完整源码、构建脚本与许可证文本随发布
包和公开仓库一并提供。
