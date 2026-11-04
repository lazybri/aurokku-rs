# Windows Qt 6 安装包（NSIS，内测）

本文面向内测用户与维护者，说明 auRokku（Qt6）Windows 安装包的获取、安装、
静默部署、升级、卸载和问题反馈方式。安装包与原有的 portable zip 并存：
zip 适合绿色/免安装使用，setup.exe 适合普通用户（开始菜单、卸载项齐全）。

## 1. 获取构件

安装包是 CI 的 workflow artifact，不进入 Releases 页面：

- 内测（beta）通道：在 Actions 页面手动触发 **Release** 工作流，
  选择 mode = beta 与分支；构建完成后在该 run 的 artifact
  `windows-x86_64-beta` 中下载。
- 命令行（推荐，方便脚本化）：

  ```bat
  gh run list -R lazybri/aurokku-rs --workflow Release --limit 10
  gh run download -R lazybri/aurokku-rs <run-id> -n windows-x86_64-beta -D dist\beta
  ```

- 普通 push 的 artifact 名为 `windows-x86_64-debug`（debug 构建），
  release 模式的 artifact 名为 `windows-x86_64`。
- 每次 Windows 构件同时包含 portable zip
  (`auroku-windows-x86_64-v<版本>.zip`) 和安装包
  (`auroku-windows-x86_64-v<版本>-setup.exe`)，另有 `.sha256` 校验文件。

> 署名说明：内测安装包未做代码签名，首次运行可能出现 SmartScreen
> “Windows 已保护你的电脑”。点击“更多信息 -> 仍要运行”即可；如需校验，
> 用 `.sha256` 文件比对 SHA256。

## 2. 安装

双击 `auroku-windows-x86_64-v<版本>-setup.exe`，按向导操作：

1. 欢迎页 -> 许可页（项目 LICENSE + libwdi LGPLv3 + 第三方声明）-> 安装 -> 完成；
2. 安装位置固定为当前用户目录，不需要管理员权限：

   ```
   %LOCALAPPDATA%\Programs\auroku
   ```

3. 开始菜单项：`auRokku`（GUI）与 `命令行`（auroku.exe）；
4. 完成页可勾选“运行 auRokku（Qt6）”；
5. 安装器检测系统 UI 语言，自动使用简体中文或英文界面。

安装目录内容与 portable zip 的 GUI 目录一致（平铺）：

- `auroku-gui.exe`：Qt 6 GUI；`auroku.exe`：CLI 与提权 helper；
- `Qt6*.dll` 与 `platforms/` 等插件目录：Qt 运行时；
- MSVC 构建额外包含 VC++ 运行时（`msvcp140.dll`、`vcruntime140.dll` 等）；
- `LICENSE`（项目许可证全文）、`LICENSE-LGPL-3.0.txt`（libwdi，仅静态链接时）、
  `THIRD-PARTY-NOTICES.txt`；
- `install-info.txt`：版本、架构、构建时间、payload SHA256、安装时间；
- `uninstall.exe`：卸载程序。

### 静默安装

  ```bat
  auroku-windows-x86_64-v0.3.2-qt6-setup.exe /S /D=C:\tools\auroku
  ```

- `/S`：静默安装，不显示任何界面（不会执行完成页的“运行 GUI”）；
- `/D=<目录>`：覆盖安装目录，必须是**最后一个**参数；
- `/DESKTOP`：额外创建桌面快捷方式（默认不创建）；
- 静默模式没有“安装位置”页，出错时不会弹窗，请查看安装日志。

## 3. 升级

直接运行新版本 setup.exe 覆盖安装即可：

- 默认沿用上次的安装目录（注册表 `HKCU\Software\auRokku\InstallDir`）；
- 安装器会先检测并在静默模式下自动关闭正在运行的 `auroku-gui.exe`
  （交互模式会提示先关闭窗口）；
- 用户数据（`%LOCALAPPDATA%\auroku`）不受升级影响；
- 开始菜单快捷方式与卸载项会被刷新，不会产生重复条目；
- 若设备当前使用 WinUSB 驱动，升级不会自动恢复原驱动；如需恢复，
  升级后在 GUI 概览页点“恢复原驱动”。

## 4. 卸载

方式一：设置 -> 应用 -> 已安装的应用 -> `auRokku (Qt6)` -> 卸载。

方式二：开始菜单 `auRokku` 文件夹中的“卸载 auRokku”。

方式三（静默）：

  ```bat
  "%LOCALAPPDATA%\Programs\auroku\uninstall.exe" /S
  ```

- 卸载会删除安装目录、开始菜单项、桌面快捷方式（如果有）和 HKCU
  卸载注册项；
- **默认保留** `%LOCALAPPDATA%\auroku`（`gui.log`、`driver.log`、
  `driver-state.json` 等用户数据），方便排查问题或重装后续用；
- 需要一并清理用户数据时使用 `/PURGEDATA`：

  ```bat
  "%LOCALAPPDATA%\Programs\auroku\uninstall.exe" /S /PURGEDATA
  ```

- 交互式卸载时，如果检测到 `driver-state.json`（曾安装过 WinUSB 驱动），
  会提醒先在 GUI 中“恢复原驱动”再卸载；静默卸载不会弹窗，直接继续。
- 静默卸载如果检测到 GUI 仍在运行，会强制关闭它后再删除文件。

## 5. 日志与诊断

| 文件 | 位置 | 说明 |
| --- | --- | --- |
| 安装/卸载日志 | %TEMP%\auroku-setup-<时间戳>.log | 安装与卸载各写一份，含版本、目录、结果 |
| 安装信息 | <安装目录>\install-info.txt | 版本、架构、构建时间、payload SHA256 |
| GUI 日志 | %LOCALAPPDATA%\auroku\gui.log | 每次启动重写，含 Qt 启动失败信息 |
| 驱动日志 | %LOCALAPPDATA%\auroku\driver.log | WinUSB 安装/恢复步骤 |
| 驱动状态 | %LOCALAPPDATA%\auroku\driver-state.json | 是否有待恢复的驱动安装 |

`%LOCALAPPDATA%\auroku` 可用环境变量 `AUROKU_STATE_DIR` 覆盖（GUI/CLI 均支持）。

## 6. 内测反馈模板

请在 issue / 群反馈中附上：

1. **Windows 版本**：`winver` 的版本与内部版本号（如 Windows 11 23H2
   22631.4460），以及是否 ARM64；
2. **安装包版本**：setup 文件名 + `<安装目录>\install-info.txt` 全文；
3. **安装日志**：`%TEMP%\auroku-setup-<时间戳>.log` 最新一份（安装与卸载各一份）；
4. **运行日志**：`%LOCALAPPDATA%\auroku\gui.log`，涉及驱动问题时加
   `driver.log` 与 `driver-state.json`；
5. **问题现象与复现步骤**：在哪一步失败、是否可稳定复现、是否只在安装版出现
   （可对比 portable zip 是否同样复现）；
6. 与驱动相关时说明：设备 VID/PID、当前设备管理器中的驱动（usbser/WinUSB）、
   是否点过“安装 WinUSB 驱动”/“恢复原驱动”；
7. 卸载相关问题请说明是否使用 `/S`、`/PURGEDATA`，以及卸载后残留的目录/快捷方式。

## 7. 维护者：构建与冒烟测试

  ```powershell
  # 1) 构建 CLI + GUI payload（MSVC 在 CI；本地可用 MinGW Qt 验证安装器逻辑）
  .\.github\scripts\build-windows.ps1 -Target x86_64-pc-windows-gnu

  # 2) 生成 per-user 安装包（含许可页合成、payload SHA256、EstimatedSize）
  .\.github\scripts\build-installer.ps1 -Version 0.3.2-qt6 -PayloadDir dist\auroku-gui-windows-x86_64 -OutFile dist\auroku-windows-x86_64-v0.3.2-qt6-setup.exe -Arch x86_64

  # 3) 静默安装 -> 安装后 GUI --smoke-test -> 静默卸载 全链路断言
  .\.github\scripts\test-installer.ps1 -SetupExe dist\auroku-windows-x86_64-v0.3.2-qt6-setup.exe
  ```

- 本机没有 MSVC 时只能验证安装器逻辑；MSVC 侧的 VC++ 运行时打包由 CI 的
  `-ExpectMsvcRuntime` 冒烟步骤断言；
- `test-installer.ps1` 默认使用 `%TEMP%\auroku-inst-test`，不会碰真实安装目录；
  加 `-TestPurge` 会额外验证 `/DESKTOP` 与 `/PURGEDATA`（会先备份并恢复
  `%LOCALAPPDATA%\auroku`）；
- CI（`.github/workflows/build.yml`）的 Windows job 在 push（debug）与
  workflow_dispatch（release/beta/debug）都会构建安装包并跑安装/卸载冒烟。

## 8. 已知限制与已知问题

- **许可证元数据不一致（既有问题，非本安装包引入）**：仓库根 `LICENSE` 是
  GNU GPL v3 全文（commit 9e0de46 "add GPL3 license"），但 `Cargo.toml` 的
  `license` 字段仍为 `MIT`。本内测安装包以仓库 `LICENSE` 原文为准：
  安装器许可页按文件内容标注为 GPLv3，`THIRD-PARTY-NOTICES.txt` 使用中性
  表述。发布说明请勿声明 MIT；等用户/法务确认后再统一元数据（届时另开提交）。
- 安装包未签名（内测阶段），SmartScreen 会提示；
- per-user 安装：只对当前用户生效，多用户需各自安装；
- 不写 HKLM、不注册协议/文件关联、不创建服务；
- Qt 6 官方最低要求 Windows 10（1809+），更老系统不受支持；
- 卸载程序不会自动恢复 WinUSB 驱动（需要先在 GUI 中恢复）；
- 静默卸载会强制结束正在运行的 GUI，可能丢失未保存的界面状态（设备操作均已完成
  才写日志，正常使用无影响）。