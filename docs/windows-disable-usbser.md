# 临时禁用 usbser.sys 内核服务（Windows，仅测试）

本目录提供两个等效工具，用于在测试时阻止 Windows 把 CDC 设备绑定到
**Microsoft USB Serial Driver (`usbser.sys`)**：

| 文件 | 用途 |
| --- | --- |
| `tools/windows/disable-usbser.reg` | 直接导入的注册表条目：`HKLM\SYSTEM\CurrentControlSet\Services\usbser\Start = 4`（`SERVICE_DISABLED`） |
| `tools/windows/usbser-service.ps1` | 推荐方式：带备份/恢复、状态查询和 `-WhatIf` 的 PowerShell 脚本 |

> **风险提示**
> - 这会影响整台机器上**所有** CDC 串口设备（普通 COM 口会停止工作），
>   只应在开发/测试环境临时使用，测试完务必恢复。
> - 修改需要管理员权限；修改后需要**重启 Windows 或重新插拔设备**才生效。
> - 禁用 `usbser.sys` **不会安装 WinUSB**，也不能让 libusb 直接打开设备；
>   它只是阻止 Windows 自动绑定 usbser，常用于排查/腾出设备后再安装其它驱动。
> - 不要在正在使用串口设备（调试器、GPS、串口服务器等）的机器上长期禁用。

## 1. GUI 按钮（推荐）

Windows 上打开“概览”页的 **WinUSB 驱动** 卡片，底部有：

- **禁用 usbser.sys（测试）** — 确认后弹一次 UAC，把 `usbser` 的 `Start`
  改为 `4`，并把原值保存到 `%LOCALAPPDATA%\auroku\usbser-state.json`；
- **恢复 usbser.sys** — 把保存的原值写回（没有保存值时恢复为 Windows 默认
  的 `3`）。

执行后同样需要重启或重新插拔设备才生效。GUI 现在通过 Windows PnP 树补充
枚举没有可用驱动的设备，所以禁用 `usbser` 后仍能在设备列表里看到
VID/PID，并从同一张卡片安装 WinUSB。

## 2. PowerShell 脚本

在**管理员权限**的 PowerShell 中运行：

```powershell
# 查看当前值、服务配置和备份状态（不需要管理员）
.\tools\windows\usbser-service.ps1 -Action Status

# 禁用：先备份原来的 Start 值，再写入 Start=4
.\tools\windows\usbser-service.ps1 -Action Disable

# 预演，不真正改注册表
.\tools\windows\usbser-service.ps1 -Action Disable -WhatIf

# 恢复：使用备份里的原值；没有备份时需显式 -Force -FallbackStart 3
.\tools\windows\usbser-service.ps1 -Action Restore
```

如果当前 PowerShell 禁止运行脚本，可只对本次进程放开：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
```

备份文件为 `%ProgramData%\auroku\usbser-service.json`，记录原来的
`Start` 值、时间和用户。`Restore` 成功后会删除该文件。

Windows 默认值为 `3`（`DEMAND_START`）。只有在**没有备份**且显式传入
`-Force` 时才会使用 `-FallbackStart`（默认 3），避免误恢复成错误值。

## 3. 注册表文件（手动导入）

1. 双击 `tools/windows/disable-usbser.reg`，在 UAC 提示中确认；
2. 或用管理员命令行导入：

   ```bat
   reg import tools\windows\disable-usbser.reg
   ```

恢复默认（demand start）：

```bat
reg add "HKLM\SYSTEM\CurrentControlSet\Services\usbser" /v Start /t REG_DWORD /d 3 /f
```

如果你用脚本禁用，请优先用脚本的 `-Action Restore`，它会恢复**原始值**
而不是假定默认值。

## 4. 验证

```bat
reg query "HKLM\SYSTEM\CurrentControlSet\Services\usbser" /v Start
sc.exe qc usbser
```

禁用后应看到 `Start = 0x4`，`sc qc` 显示 `START_TYPE : 4 DISABLED`；恢复后
应回到原来的 `3 DEMAND_START`（或备份中的值）。

## 5. 与 auRokku 的 WinUSB 流程配合

- 测试某台被 `usbser.sys` 占用的设备时，可以先禁用 usbser、重启/重插，
  让 Windows 不再自动绑定串口驱动，再按 [windows-driver.md](windows-driver.md)
  的方式安装 WinUSB；也可以反过来先恢复 usbser 再让设备回到正常 COM 口。
- 禁用 usbser 不等同于安装 WinUSB：设备如果没有其它驱动，libusb 仍无法打开，
  所以通常还需要安装 WinUSB（发布版 GUI 概览页的常驻驱动卡）或使用
  [Zadig / 签名 INF](windows-driver.md) 流程。
- GUI 的“WinUSB 驱动”卡可以随时恢复原始驱动；本工具只调整 `usbser` 服务本身，
  与驱动的“退出时自动恢复”是两件事。

## 6. 相关文件

- [tools/windows/disable-usbser.reg](../tools/windows/disable-usbser.reg)
- [tools/windows/usbser-service.ps1](../tools/windows/usbser-service.ps1)
- [docs/windows-driver.md](windows-driver.md)
