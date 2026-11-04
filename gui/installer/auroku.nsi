; ---------------------------------------------------------------------------
; auRokku (Qt6) install script - per-user, single-file NSIS package
;
; Built by .github/scripts/build-installer.ps1, which passes these defines:
;   APP_VERSION        display version, e.g. 0.3.2-qt6
;   APP_VERSION_4      4-part numeric version for VIProductVersion, e.g. 0.3.2.0
;   PAYLOAD_DIR        dist\auroku-gui-windows-<arch> (GUI + helper + Qt + licenses)
;   OUT_FILE           path of the produced setup executable
;   ESTIMATED_SIZE_KB  payload size in KiB (Add/Remove Programs)
;   LICENSE_FILE       combined LICENSE + LGPLv3 + third-party notices text
;   BUILD_ARCH         e.g. x86_64
;   BUILD_TIME         ISO-8601 build timestamp
;   PAYLOAD_SHA256     manifest SHA256 of the payload
;
; The installer is per-user (no admin): it installs to
; %LOCALAPPDATA%\Programs\auroku and registers a HKCU uninstall entry. Driver
; installation is still triggered by auroku.exe, which asks for one UAC prompt.
;
; IMPORTANT: this file is saved as UTF-8 *with* BOM; NSIS 3 needs the BOM to
; parse the Chinese language strings below.
; ---------------------------------------------------------------------------

Unicode true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

; Language IDs used by LangString / VIAddVersionKey / $LANGUAGE. The NSIS
; built-in LANG_* defines only exist after a language file is loaded, which MUI
; does after the pages; numeric IDs keep the definitions order-independent.
!define LANG_EN 1033
!define LANG_ZH 2052

; ---------------------------------------------------------------------------
; Build-time definitions
; ---------------------------------------------------------------------------
!ifndef APP_VERSION
  !error "APP_VERSION is required (/DAPP_VERSION=<display version>)"
!endif
!ifndef APP_VERSION_4
  !error "APP_VERSION_4 is required (/DAPP_VERSION_4=<numeric 4-part version>)"
!endif
!ifndef PAYLOAD_DIR
  !error "PAYLOAD_DIR is required (/DPAYLOAD_DIR=<payload directory>)"
!endif
!ifndef OUT_FILE
  !error "OUT_FILE is required (/DOUT_FILE=<setup executable>)"
!endif
!ifndef LICENSE_FILE
  !error "LICENSE_FILE is required (/DLICENSE_FILE=<combined license text>)"
!endif
!ifndef ESTIMATED_SIZE_KB
  !define ESTIMATED_SIZE_KB 0
!endif
!ifndef BUILD_ARCH
  !define BUILD_ARCH "x86_64"
!endif
!ifndef BUILD_TIME
  !define BUILD_TIME "unknown"
!endif
!ifndef PAYLOAD_SHA256
  !define PAYLOAD_SHA256 "unknown"
!endif

!define APP_NAME "auRokku"
!define APP_DISPLAY_NAME "auRokku (Qt6)"
!define APP_EXE "auroku-gui.exe"
!define CLI_EXE "auroku.exe"
!define UNINST_EXE "uninstall.exe"
!define APP_REG_KEY "Software\auRokku"
!define UNINSTALL_REG_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\auRokkuQt6"
!define STARTMENU_FOLDER "$SMPROGRAMS\auRokku"
!define DATA_DIR "$LOCALAPPDATA\auroku"

!if ! /FileExists "${PAYLOAD_DIR}\${APP_EXE}"
  !error "payload is missing ${APP_EXE}: check -PayloadDir"
!endif

ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma
CRCCheck on

Name "${APP_DISPLAY_NAME} ${APP_VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\auroku"
InstallDirRegKey HKCU "${APP_REG_KEY}" "InstallDir"
BrandingText "${APP_DISPLAY_NAME} ${APP_VERSION}"

VIProductVersion "${APP_VERSION_4}"
VIAddVersionKey /LANG=${LANG_EN} "ProductName" "${APP_DISPLAY_NAME}"
VIAddVersionKey /LANG=${LANG_EN} "FileDescription" "${APP_DISPLAY_NAME} Setup"
VIAddVersionKey /LANG=${LANG_EN} "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=${LANG_EN} "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=${LANG_EN} "CompanyName" "auRokku project"
VIAddVersionKey /LANG=${LANG_EN} "LegalCopyright" "See LICENSE"
VIAddVersionKey /LANG=${LANG_ZH} "ProductName" "${APP_DISPLAY_NAME}"
VIAddVersionKey /LANG=${LANG_ZH} "FileDescription" "${APP_DISPLAY_NAME} 安装程序"
VIAddVersionKey /LANG=${LANG_ZH} "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=${LANG_ZH} "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=${LANG_ZH} "CompanyName" "auRokku project"
VIAddVersionKey /LANG=${LANG_ZH} "LegalCopyright" "See LICENSE"

; ---------------------------------------------------------------------------
; Modern UI configuration
; ---------------------------------------------------------------------------
!define MUI_ABORTWARNING
!define MUI_UNABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "$(WelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!define MUI_LICENSEPAGE_TEXT_TOP "$(LicenseTop)"
!define MUI_FINISHPAGE_TITLE "$(FinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(FinishText)"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "$(RunAppText)"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "$(UninstallConfirmText)"

; ---------------------------------------------------------------------------
; Localised strings
; ---------------------------------------------------------------------------
LangString WelcomeTitle ${LANG_EN} "Welcome to the auRokku (Qt6) Setup"
LangString WelcomeTitle ${LANG_ZH} "欢迎使用 auRokku（Qt6）安装程序"

LangString WelcomeText ${LANG_EN} "This installs auRokku (Qt6) version ${APP_VERSION} for the current user only.$\r$\n$\r$\nNo administrator rights are needed: the program is installed into your local AppData\Programs\auroku folder. The WinUSB driver is installed later from the GUI with one UAC confirmation."
LangString WelcomeText ${LANG_ZH} "本程序为当前用户安装 auRokku（Qt6）${APP_VERSION}。$\r$\n$\r$\n本安装不需要管理员权限：程序安装在当前用户的 AppData\Programs\auroku 目录。WinUSB 驱动稍后在 GUI 中安装，只需要确认一次 UAC。"

LangString LicenseTop ${LANG_EN} "Please review the license terms before installing auRokku (Qt6). The text below includes the auRokku license (repository LICENSE), the LGPLv3 text of libwdi and the third-party notices."
LangString LicenseTop ${LANG_ZH} "安装 auRokku（Qt6）之前，请阅读以下许可条款。下方文本包含 auRokku 的许可证全文、libwdi 的 LGPLv3 文本以及第三方声明。"

LangString FinishTitle ${LANG_EN} "auRokku (Qt6) has been installed"
LangString FinishTitle ${LANG_ZH} "auRokku（Qt6）安装完成"

LangString FinishText ${LANG_EN} "auRokku (Qt6) ${APP_VERSION} is installed for the current user.$\r$\n$\r$\nThe installation log is in the TEMP folder (auroku-setup-*.log). You can start the GUI from the Start menu or the checkbox below."
LangString FinishText ${LANG_ZH} "已为当前用户安装 auRokku（Qt6）${APP_VERSION}。$\r$\n$\r$\n安装日志位于 TEMP 目录（auroku-setup-*.log）。可以从开始菜单或下方选项启动 GUI。"

LangString RunAppText ${LANG_EN} "Run auRokku (Qt6)"
LangString RunAppText ${LANG_ZH} "运行 auRokku（Qt6）"

LangString ShortcutCliText ${LANG_EN} "Command Line"
LangString ShortcutCliText ${LANG_ZH} "命令行"

LangString UninstallShortcutText ${LANG_EN} "Uninstall auRokku"
LangString UninstallShortcutText ${LANG_ZH} "卸载 auRokku"

LangString SectionMainName ${LANG_EN} "auRokku (Qt6) application"
LangString SectionMainName ${LANG_ZH} "auRokku（Qt6）程序文件"

LangString UninstallConfirmText ${LANG_EN} "This removes auRokku (Qt6) from the current user account.$\r$\n$\r$\nYour data in local AppData\auroku (log files and driver state) is kept. Pass /PURGEDATA to the uninstaller to remove it as well."
LangString UninstallConfirmText ${LANG_ZH} "将从当前用户账户中卸载 auRokku（Qt6）。$\r$\n$\r$\n本地 AppData\auroku 中的用户数据（日志与驱动状态）会被保留。若要一并删除，请向卸载程序传入 /PURGEDATA。"

LangString GuiRunningText ${LANG_EN} "auRokku (Qt6) is still running.$\r$\n$\r$\nClose the application window and click OK to retry, or Cancel to abort the uninstallation."
LangString GuiRunningText ${LANG_ZH} "auRokku（Qt6）仍在运行。$\r$\n$\r$\n请关闭程序窗口后点击“确定”重试，或点击“取消”中止卸载。"

LangString GuiRunningInstallText ${LANG_EN} "auRokku (Qt6) is still running. A silent upgrade closes it automatically; otherwise close the window and click OK to retry, or Cancel to abort the installation."
LangString GuiRunningInstallText ${LANG_ZH} "auRokku（Qt6）仍在运行。静默升级会自动关闭它；否则请关闭程序窗口后点击“确定”重试，或点击“取消”中止安装。"

LangString DriverStateText ${LANG_EN} "A WinUSB driver installation is recorded in local AppData\auroku\driver-state.json.$\r$\n$\r$\nIt is recommended to restore the original driver from the GUI before uninstalling (Overview -> WinUSB driver -> Restore).$\r$\n$\r$\nContinue with the uninstallation?"
LangString DriverStateText ${LANG_ZH} "检测到本地 AppData\auroku\driver-state.json 中记录了 WinUSB 驱动安装状态。$\r$\n$\r$\n建议先在 GUI 中恢复原驱动再卸载（概览页 -> WinUSB 驱动 -> 恢复原驱动）。$\r$\n$\r$\n仍要继续卸载吗？"

; ---------------------------------------------------------------------------
; Pages
; ---------------------------------------------------------------------------
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages must be inserted after the page macros. English is the fallback;
; .onInit switches to Simplified Chinese when the system UI is Chinese.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------------------------------------------------------------------------
; Variables
; ---------------------------------------------------------------------------
Var DesktopShortcut
Var Stamp
Var LogPath
Var UnStamp
Var UnLogPath
Var UnPurgeData

; ---------------------------------------------------------------------------
; Helpers
; ---------------------------------------------------------------------------
; Builds a "YYYYMMDD-HHMMSS" local timestamp in $Stamp (installer) or
; $UnStamp (uninstaller). $1..$8 are scratch registers.
Function GetStamp
  System::Alloc 16
  Pop $0
  System::Call 'kernel32::GetLocalTime(p)i(r0)'
  System::Call '*$0(&i2.r1,&i2.r2,&i2.r3,&i2.r4,&i2.r5,&i2.r6,&i2.r7,&i2.r8)'
  System::Free $0
  IntFmt $1 "%04u" $1
  IntFmt $2 "%.2u" $2
  IntFmt $4 "%.2u" $4
  IntFmt $5 "%.2u" $5
  IntFmt $6 "%.2u" $6
  IntFmt $7 "%.2u" $7
  StrCpy $Stamp "$1$2$4-$5$6$7"
FunctionEnd

Function un.GetStamp
  System::Alloc 16
  Pop $0
  System::Call 'kernel32::GetLocalTime(p)i(r0)'
  System::Call '*$0(&i2.r1,&i2.r2,&i2.r3,&i2.r4,&i2.r5,&i2.r6,&i2.r7,&i2.r8)'
  System::Free $0
  IntFmt $1 "%04u" $1
  IntFmt $2 "%.2u" $2
  IntFmt $4 "%.2u" $4
  IntFmt $5 "%.2u" $5
  IntFmt $6 "%.2u" $6
  IntFmt $7 "%.2u" $7
  StrCpy $UnStamp "$1$2$4-$5$6$7"
FunctionEnd

; Sets $R4 to "1" when the option string in $R3 occurs in the parameters
; string in $R2, otherwise "0". FileFunc's GetOptions returns the text after
; the option rather than a flag, so this tiny matcher keeps /DESKTOP and
; /PURGEDATA predictable.
Function HasOption
  StrLen $0 $R3
  StrLen $1 $R2
  StrCpy $2 0
  has_option_loop:
    IntCmp $2 $1 has_option_no
    StrCpy $5 $R2 $0 $2
    StrCmp $5 $R3 has_option_yes
    IntOp $2 $2 + 1
    Goto has_option_loop
  has_option_yes:
    StrCpy $R4 "1"
    Return
  has_option_no:
    StrCpy $R4 "0"
FunctionEnd

Function un.HasOption
  StrLen $0 $R3
  StrLen $1 $R2
  StrCpy $2 0
  un_has_option_loop:
    IntCmp $2 $1 un_has_option_no
    StrCpy $5 $R2 $0 $2
    StrCmp $5 $R3 un_has_option_yes
    IntOp $2 $2 + 1
    Goto un_has_option_loop
  un_has_option_yes:
    StrCpy $R4 "1"
    Return
  un_has_option_no:
    StrCpy $R4 "0"
FunctionEnd

; ---------------------------------------------------------------------------
; Init
; ---------------------------------------------------------------------------
Function .onInit
  ; Per-user installation: Start menu / desktop resolve to the current user.
  SetShellVarContext current
  StrCpy $DesktopShortcut "0"

  ; Detect the system UI language: Chinese (any) -> Simplified Chinese,
  ; everything else -> English. No language selection dialog is shown.
  System::Call 'kernel32::GetUserDefaultUILanguage()i.r0'
  IntOp $0 $0 & 0x3FF
  StrCmp $0 0x04 0 +2
    StrCpy $LANGUAGE ${LANG_ZH}

  ; Optional desktop shortcut: /DESKTOP
  ${GetParameters} $R0
  StrCpy $R2 $R0
  StrCpy $R3 "/DESKTOP"
  Call HasOption
  StrCmp $R4 "1" 0 +2
    StrCpy $DesktopShortcut "1"
FunctionEnd

Function un.onInit
  SetShellVarContext current
  StrCpy $UnPurgeData "0"

  System::Call 'kernel32::GetUserDefaultUILanguage()i.r0'
  IntOp $0 $0 & 0x3FF
  StrCmp $0 0x04 0 +2
    StrCpy $LANGUAGE ${LANG_ZH}

  ; Optional user-data purge: uninstall.exe /PURGEDATA
  ${GetParameters} $R0
  StrCpy $R2 $R0
  StrCpy $R3 "/PURGEDATA"
  Call un.HasOption
  StrCmp $R4 "1" 0 +2
    StrCpy $UnPurgeData "1"
FunctionEnd

; ---------------------------------------------------------------------------
; Install
; ---------------------------------------------------------------------------
Section "$(SectionMainName)" SecMain
  SectionIn RO
  SetShellVarContext current
  SetOutPath "$INSTDIR"

  ; NSIS file mode "a" rewrites from the start of the file instead of
  ; appending, so the log handle stays open until the section is done.
  Call GetStamp
  StrCpy $LogPath "$TEMP\auroku-setup-$Stamp.log"
  FileOpen $9 "$LogPath" w
  FileWrite $9 "auRokku (Qt6) installer ${APP_VERSION}$\r$\n"
  FileWrite $9 "build: ${BUILD_TIME}$\r$\n"
  FileWrite $9 "arch: ${BUILD_ARCH}$\r$\n"
  FileWrite $9 "payload sha256: ${PAYLOAD_SHA256}$\r$\n"
  FileWrite $9 "timestamp: $Stamp$\r$\n"
  FileWrite $9 "install dir: $INSTDIR$\r$\n"

  ; -- Running GUI check -----------------------------------------------------
  ; Upgrading over a running instance would fail to overwrite locked files.
  ; Interactive installs ask the user to close it; /S closes it silently.
  install_gui_check:
    nsExec::Exec 'cmd /c tasklist /FI "IMAGENAME eq ${APP_EXE}" /NH | findstr /I "${APP_EXE}"'
    Pop $0
    StrCmp $0 0 0 install_gui_done
    IfSilent install_gui_kill
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "$(GuiRunningInstallText)" IDOK install_gui_check IDCANCEL install_gui_abort
  install_gui_kill:
    nsExec::Exec 'taskkill /F /IM ${APP_EXE} /T'
    Sleep 1000
    Goto install_gui_done
  install_gui_abort:
    FileWrite $9 "install: aborted, GUI still running$\r$\n"
    FileClose $9
    Quit
  install_gui_done:

  ; The payload is exactly dist\auroku-gui-windows-<arch>\*.* : GUI, auroku.exe
  ; helper, Qt runtime/plugins, VC++ runtime and license files.
  File /r "${PAYLOAD_DIR}\*.*"

  ; Machine-readable install info for testers and support.
  FileOpen $0 "$INSTDIR\install-info.txt" w
  FileWrite $0 "auRokku (Qt6)$\r$\n"
  FileWrite $0 "Version: ${APP_VERSION}$\r$\n"
  FileWrite $0 "Architecture: ${BUILD_ARCH}$\r$\n"
  FileWrite $0 "Build time: ${BUILD_TIME}$\r$\n"
  FileWrite $0 "Payload SHA256: ${PAYLOAD_SHA256}$\r$\n"
  FileWrite $0 "Install time: $Stamp$\r$\n"
  FileClose $0

  WriteUninstaller "$INSTDIR\${UNINST_EXE}"

  ; Start menu shortcuts (per-user because of SetShellVarContext current).
  CreateDirectory "${STARTMENU_FOLDER}"
  CreateShortcut "${STARTMENU_FOLDER}\auRokku.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "${STARTMENU_FOLDER}\$(ShortcutCliText).lnk" "$INSTDIR\${CLI_EXE}" "" "$INSTDIR\${CLI_EXE}" 0
  CreateShortcut "${STARTMENU_FOLDER}\$(UninstallShortcutText).lnk" "$INSTDIR\${UNINST_EXE}"
  StrCmp $DesktopShortcut "1" 0 +2
    CreateShortcut "$DESKTOP\auRokku.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  ; Uninstall entry for Settings > Apps / Apps & features.
  WriteRegStr HKCU "${APP_REG_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${APP_REG_KEY}" "Version" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "DisplayName" "${APP_DISPLAY_NAME}"
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "Publisher" "auRokku project"
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "${UNINSTALL_REG_KEY}" "EstimatedSize" ${ESTIMATED_SIZE_KB}
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "UninstallString" '"$INSTDIR\${UNINST_EXE}"'
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINST_EXE}" /S'
  WriteRegDWORD HKCU "${UNINSTALL_REG_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_REG_KEY}" "NoRepair" 1
  WriteRegStr HKCU "${UNINSTALL_REG_KEY}" "URLInfoAbout" "https://github.com/lazybri/aurokku-rs"

  FileWrite $9 "install: ok$\r$\n"
  FileClose $9
SectionEnd

; ---------------------------------------------------------------------------
; Uninstall
; ---------------------------------------------------------------------------
Section "Uninstall"
  SetShellVarContext current

  ; Keep the log handle open - see the note in the install section.
  Call un.GetStamp
  StrCpy $UnLogPath "$TEMP\auroku-setup-$UnStamp.log"
  FileOpen $9 "$UnLogPath" w
  FileWrite $9 "auRokku (Qt6) uninstaller ${APP_VERSION}$\r$\n"
  FileWrite $9 "timestamp: $UnStamp$\r$\n"
  FileWrite $9 "install dir: $INSTDIR$\r$\n"

  ; -- Running GUI check -----------------------------------------------------
  ; tasklist via findstr is enough here: exit code 0 means the process is up.
  gui_check:
    nsExec::Exec 'cmd /c tasklist /FI "IMAGENAME eq ${APP_EXE}" /NH | findstr /I "${APP_EXE}"'
    Pop $0
    StrCmp $0 0 0 gui_check_done
    IfSilent gui_check_kill
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "$(GuiRunningText)" IDOK gui_check IDCANCEL gui_check_abort
  gui_check_kill:
    nsExec::Exec 'taskkill /F /IM ${APP_EXE} /T'
    Sleep 1000
    Goto gui_check_done
  gui_check_abort:
    FileWrite $9 "uninstall: aborted, GUI still running$\r$\n"
    FileClose $9
    Quit
  gui_check_done:

  ; -- Driver-state reminder -------------------------------------------------
  ; Silent uninstall must never block on a MessageBox.
  IfFileExists "${DATA_DIR}\driver-state.json" 0 driver_hint_done
  IfSilent driver_hint_done
  MessageBox MB_YESNO|MB_ICONEXCLAMATION "$(DriverStateText)" IDYES driver_hint_done IDNO driver_hint_abort
  driver_hint_abort:
    FileWrite $9 "uninstall: aborted by user (driver state reminder)$\r$\n"
    FileClose $9
    Quit
  driver_hint_done:

  ; -- Shortcuts -------------------------------------------------------------
  Delete "$DESKTOP\auRokku.lnk"
  Delete "${STARTMENU_FOLDER}\*.lnk"
  RMDir "${STARTMENU_FOLDER}"

  ; -- Registry --------------------------------------------------------------
  DeleteRegKey HKCU "${UNINSTALL_REG_KEY}"
  DeleteRegKey HKCU "${APP_REG_KEY}"

  ; -- Files -----------------------------------------------------------------
  RMDir /r "$INSTDIR"

  ; -- Optional user-data purge (/PURGEDATA) ---------------------------------
  StrCmp $UnPurgeData "1" 0 purge_done
    RMDir /r "${DATA_DIR}"
  purge_done:

  FileWrite $9 "uninstall: ok (user data kept unless /PURGEDATA)$\r$\n"
  FileClose $9
SectionEnd
