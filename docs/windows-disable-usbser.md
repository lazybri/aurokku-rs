# 阻止 usbser.sys 首次安装（Ports 类策略，Windows 仅测试）

这份文档说明为什么“只禁用 usbser 服务”无法阻止设备第一次接入 Windows 时被
`usbser.sys` 接管，以及当前 GUI/CLI 使用的替代方案：**在设备第一次插入前，
通过 Windows 设备安装限制策略阻止 Ports 类驱动安装**，让 `usbser.inf` 无法安装。

## 0. 为什么不能只禁用 usbser 服务

`HKLM\SYSTEM\CurrentControlSet\Services\usbser\Start = 4` 只控制“服务是否允许
启动”，不改变 PnP 的驱动选择。设备第一次安装时，Windows 按兼容 ID
`USB\Class_02&SubClass_02&Prot_01` 选中系统自带的 `usbser.inf`，并在安装过程中
执行 INF 的 `AddService=usbser,...,StartType=3`，**把 Start 改回 3 并启动驱动**。
所以先禁用服务、再插设备，服务会被设备安装过程重新启用。

典型证据（`C:\Windows\INF\setupapi.dev.log`）：

```text
[Device Install (Hardware initiated) - USB\VID_xxxx&PID_yyyy\...]
Driver INF     - usbser.inf
Configuration  - USB\Class_02&SubClass_02&Prot_01
StartType=3    (usbser.inf line 80)
Add Service: Modified existing service 'usbser'.
Install Device: Starting device ... completed.
```

对会因 usbser 初始化而无限重启的品牌（例如部分 Sanyo/au 设备），必须在
**任何驱动安装之前**阻止 Ports 类安装，而不是事后改服务。

## 1. GUI 按钮（推荐）

Windows 上打开“概览”页的 **WinUSB 驱动** 卡片，底部有两个按钮：

- **阻止 usbser 首次安装（Ports 类策略）**
  - 确认后弹一次 UAC；
  - 添加 Windows 设备安装限制：`DenyDeviceClasses` + Ports 类 GUID
    `{4d36e978-e325-11ce-bfc1-08002be10318}`；
  - 之后新插入的 CDC 设备不会被 `usbser.inf` 安装，设备会停在“无驱动/未知设备”
    状态，从而避免 usbser 触发的重启循环；
- **恢复策略** — 安装 WinUSB 后移除该限制（恢复前会保存/导出原策略）。

操作成功后插入设备，确认设备仍在“无驱动”状态，再通过同一张卡片安装 WinUSB；
WinUSB 属于 USBDevice 类，不受 Ports 类限制影响。

## 2. CLI 等价命令

`auroku.exe` 里对应两个操作（`--vid/--pid` 是全局必填参数，但这两个操作不使用）：

```bat
:: 阻止新的 Ports 类驱动安装（需要一次 UAC）
auroku --op block-usbser-install --vid 0000 --pid 0000

:: 恢复设备安装策略
auroku --op restore-usbser-policy --vid 0000 --pid 0000
```

## 3. 策略写入的位置

```text
HKLM\SOFTWARE\Policies\Microsoft\Windows\DeviceInstall\Restrictions
  DenyDeviceClasses = 1 (DWORD)
  DenyDeviceClasses\1
    DeviceClass = {4d36e978-e325-11ce-bfc1-08002be10318} (REG_SZ)
```

- 只阻止**新安装**；已经安装的设备不会因此被卸载。
- 备份状态：`%LOCALAPPDATA%\auroku\usbser-policy-state.json`；
  如果原策略键已存在，会先导出到
  `%LOCALAPPDATA%\auroku\device-install-policy-backup.reg`。
- **恢复策略会删除整个 DenyDeviceClasses 策略值**（如果它原本存在则从备份导入），
  并在成功前不做进一步修改。

## 4. 手动应用/清除（不使用 GUI/CLI 时）

管理员 PowerShell：

```powershell
$key = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeviceInstall\Restrictions'
New-Item -Path "$key\DenyDeviceClasses" -Force | Out-Null
New-ItemProperty -Path $key -Name DenyDeviceClasses -Value 1 -PropertyType DWord -Force
New-ItemProperty -Path "$key\DenyDeviceClasses\1" -Name DeviceClass `
  -Value '{4d36e978-e325-11ce-bfc1-08002be10318}' -PropertyType String -Force
gpupdate /force
```

清除：

```powershell
Remove-Item "$key\DenyDeviceClasses" -Recurse -Force
Remove-ItemProperty -Path $key -Name DenyDeviceClasses -ErrorAction SilentlyContinue
gpupdate /force
```

## 5. 验证

1. 应用策略后，`reg query "HKLM\SOFTWARE\Policies\Microsoft\Windows\DeviceInstall\Restrictions" /s`
   应能看到 `DenyDeviceClasses` 与 Ports 类 GUID；
2. 插入设备后，设备管理器应显示“未知设备/其他设备”，Problem 为
   `CM_PROB_FAILED_INSTALL` / Code 28，而不是 `USB 串行设备 (COMx)`；
3. 此时安装 WinUSB 驱动，libusb 应能访问；
4. 安装完成后点击“恢复策略”，再插拔一次设备确认恢复正常驱动安装。

## 6. 风险

- 策略生效期间，本机**所有新插入的 Ports 类设备**（USB 转串口、调制解调器等）
  都无法安装驱动；仅建议在测试机临时使用，安装 WinUSB 后立即恢复。
- 已安装的串口设备不受影响；但不要把它当作长期生产配置。
- 策略不能替代签名：WinUSB 驱动仍然需要 testsigning 或微软 attestation/WHQL 签名。
- 若设备已被 usbser 安装过，先卸载设备节点/删除驱动，再重新插入，策略才会
  阻止下一次 usbser 安装。

## 7. 旧的服务禁用工具（仅作兼容保留）

`tools/windows/disable-usbser.reg` 与 `tools/windows/usbser-service.ps1` 仍然保留，
用于“驱动已安装、且不会再触发 INF 重装”的场景（例如已装好 WinUSB 后临时排查）。
它们**不能**阻止首次安装，不要用于会无限重启的设备。

```powershell
.\tools\windows\usbser-service.ps1 -Action Status
.\tools\windows\usbser-service.ps1 -Action Disable
.\tools\windows\usbser-service.ps1 -Action Restore
```

## 8. 相关文件

- [docs/windows-driver.md](windows-driver.md)
- [src/driver.rs](../src/driver.rs) — `block_usbser_install` / `restore_usbser_policy`
- [src/bin/auroku-gui/app.rs](../src/bin/auroku-gui/app.rs) — WinUSB 卡片按钮
- [tools/windows/disable-usbser.reg](../tools/windows/disable-usbser.reg)（旧服务方案）
- [tools/windows/usbser-service.ps1](../tools/windows/usbser-service.ps1)（旧服务方案）
