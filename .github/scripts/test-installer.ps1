<#
.SYNOPSIS
    Silent install -> installed GUI smoke -> silent uninstall test for the
    auRokku Qt 6 NSIS installer.

.DESCRIPTION
    Verifies the per-user installer end to end without any interactive UI:
      * silent install (/S /D=<dir>) and its exit code
      * installed payload (GUI, auroku.exe helper, Qt runtime/plugins, licenses)
      * install-info.txt (version, architecture, payload SHA256)
      * Start menu shortcuts and the HKCU uninstall entry
      * upgrade over a running auroku-gui.exe (silent mode closes it)
      * the installed GUI with QT_QPA_PLATFORM=offscreen --smoke-test
      * silent uninstall and the removal of every installed artifact
      * preservation of the %LOCALAPPDATA%\auroku user data directory
      * install/uninstall logs in %TEMP%

    With -TestPurge, a second install/uninstall cycle checks /DESKTOP and
    /PURGEDATA (the user data directory is backed up and restored so a
    developer machine never loses real logs or driver state).

.EXAMPLE
    .\.github\scripts\test-installer.ps1 -SetupExe dist\auroku-windows-x86_64-v0.3.2-qt6-setup.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SetupExe,

    [string]$InstallDir = '',
    [string]$Arch = 'x86_64',
    [string]$ExpectedVersion = '',
    [switch]$ExpectMsvcRuntime,
    [switch]$TestPurge
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if (-not $InstallDir) {
    $InstallDir = Join-Path $env:TEMP 'auroku-inst-test'
}
if (-not $ExpectedVersion) {
    $ExpectedVersion = (Select-String -Path (Join-Path (Get-Location) 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
}

$setup = (Resolve-Path -LiteralPath $SetupExe).Path
$installDir = [System.IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
$startMenuDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\auRokku'
$desktopLnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'auRokku.lnk'
$uninstallRegKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\auRokkuQt6'
$appRegKey = 'HKCU:\Software\auRokku'
$dataDir = Join-Path $env:LOCALAPPDATA 'auroku'
$marker = Join-Path $dataDir 'installer-smoke-marker.txt'
$smokeStateDir = Join-Path $env:TEMP 'auroku-inst-test-state'
$failures = New-Object System.Collections.Generic.List[string]

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if ($Condition) {
        Write-Host ('  PASS ' + $Message)
    } else {
        Write-Host ('  FAIL ' + $Message)
        $script:failures.Add($Message)
    }
}

function Remove-TestArtifacts {
    Remove-Item -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $startMenuDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $desktopLnk -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $uninstallRegKey -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $appRegKey -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $marker -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $smokeStateDir -Recurse -Force -ErrorAction SilentlyContinue
}

function Wait-ForPathRemoved {
    param([string]$Path, [int]$TimeoutSeconds = 90)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Test-Path -LiteralPath $Path) -and ((Get-Date) -lt $deadline)) {
        Start-Sleep -Milliseconds 250
    }
    return (-not (Test-Path -LiteralPath $Path))
}

function Start-SilentProcess {
    param([string]$FilePath, [string]$Arguments, [string]$Label)
    $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -Wait -PassThru
    Assert-True ($process.ExitCode -eq 0) ($Label + ' exit code 0 (got ' + $process.ExitCode + ')')
    return $process
}

# --- Prepare ----------------------------------------------------------------
Write-Host ('==> Installer:   ' + $setup)
Write-Host ('    install dir: ' + $installDir)
Remove-TestArtifacts
$logsBefore = @(Get-ChildItem -Path $env:TEMP -Filter 'auroku-setup-*.log' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)

$installArguments = '/S /D=' + $installDir

# --- 1. Silent install ------------------------------------------------------
Write-Host '==> Silent install (/S)'
Start-SilentProcess -FilePath $setup -Arguments $installArguments -Label 'silent install' | Out-Null

foreach ($name in @('auroku-gui.exe', 'auroku.exe', 'uninstall.exe', 'install-info.txt', 'LICENSE')) {
    Assert-True (Test-Path -LiteralPath (Join-Path $installDir $name)) ('installed: ' + $name)
}
$qtDlls = @(Get-ChildItem -LiteralPath $installDir -Filter 'Qt6*.dll' -File -ErrorAction SilentlyContinue)
Assert-True ($qtDlls.Count -gt 0) ('installed: Qt 6 runtime DLLs (' + $qtDlls.Count + ')')
Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'platforms\qwindows.dll')) 'installed: platforms\qwindows.dll'
Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'platforms\qoffscreen.dll')) 'installed: platforms\qoffscreen.dll'
Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'THIRD-PARTY-NOTICES.txt')) 'installed: THIRD-PARTY-NOTICES.txt'
Assert-True (-not (Test-Path -LiteralPath (Join-Path $installDir 'auroku_ffi.dll'))) 'no auroku_ffi.dll bridge (Rust backend is static)'
if ($ExpectMsvcRuntime) {
    Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'msvcp140.dll')) 'installed: msvcp140.dll (VC++ runtime)'
    Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'vcruntime140.dll')) 'installed: vcruntime140.dll (VC++ runtime)'
    Assert-True (Test-Path -LiteralPath (Join-Path $installDir 'vcruntime140_1.dll')) 'installed: vcruntime140_1.dll (VC++ runtime)'
}

$infoPath = Join-Path $installDir 'install-info.txt'
$info = Get-Content -LiteralPath $infoPath -Raw
Assert-True ($info -match ('Version: ' + [regex]::Escape($ExpectedVersion))) ('install-info.txt version is ' + $ExpectedVersion)
Assert-True ($info -match ('Architecture: ' + [regex]::Escape($Arch))) ('install-info.txt architecture is ' + $Arch)
Assert-True ($info -match 'Payload SHA256: [0-9a-f]{64}') 'install-info.txt records a SHA256 payload hash'

Assert-True (Test-Path -LiteralPath $startMenuDir) 'start menu folder exists'
Assert-True (Test-Path -LiteralPath (Join-Path $startMenuDir 'auRokku.lnk')) 'start menu: auRokku.lnk'
$shortcuts = @(Get-ChildItem -LiteralPath $startMenuDir -Filter '*.lnk' -ErrorAction SilentlyContinue)
Assert-True ($shortcuts.Count -ge 3) ('start menu has GUI + CLI + uninstall shortcuts (found ' + $shortcuts.Count + ')')

$props = Get-ItemProperty -Path $uninstallRegKey -ErrorAction SilentlyContinue
Assert-True ($null -ne $props) 'HKCU uninstall entry exists'
if ($null -ne $props) {
    Assert-True ($props.DisplayName -eq 'auRokku (Qt6)') ('uninstall DisplayName is auRokku (Qt6) (got ' + $props.DisplayName + ')')
    Assert-True ($props.DisplayVersion -eq $ExpectedVersion) ('uninstall DisplayVersion is ' + $ExpectedVersion + ' (got ' + $props.DisplayVersion + ')')
    Assert-True (-not [string]::IsNullOrWhiteSpace($props.Publisher)) 'uninstall Publisher is set'
    Assert-True (([string]$props.InstallLocation).TrimEnd('\') -ieq $installDir) 'uninstall InstallLocation matches the install dir'
    Assert-True ([int]$props.EstimatedSize -gt 0) ('uninstall EstimatedSize is set (' + $props.EstimatedSize + ' KiB)')
    Assert-True ([string]$props.UninstallString -match 'uninstall\.exe') 'uninstall UninstallString points at uninstall.exe'
    Assert-True ([string]$props.QuietUninstallString -match '/S') 'uninstall QuietUninstallString uses /S'
    Assert-True (($props.NoModify -eq 1) -and ($props.NoRepair -eq 1)) 'uninstall entry sets NoModify/NoRepair'
}
$appProps = Get-ItemProperty -Path $appRegKey -ErrorAction SilentlyContinue
Assert-True (($null -ne $appProps) -and ($appProps.Version -eq $ExpectedVersion)) 'HKCU Software\auRokku records the version'

# --- 2. Upgrade over a running GUI -----------------------------------------
Write-Host '==> Upgrade over a running auRokku GUI (silent mode closes it)'
$env:QT_QPA_PLATFORM = 'offscreen'
$env:AUROKU_STATE_DIR = $smokeStateDir
$runningGui = Start-Process -FilePath (Join-Path $installDir 'auroku-gui.exe') -PassThru
Start-Sleep -Seconds 3
$runningGui.Refresh()
Assert-True (-not $runningGui.HasExited) 'GUI is running before the upgrade'
Start-SilentProcess -FilePath $setup -Arguments $installArguments -Label 'silent upgrade over running GUI' | Out-Null
$guiClosed = $false
for ($attempt = 0; $attempt -lt 20; $attempt++) {
    Start-Sleep -Milliseconds 500
    $runningGui.Refresh()
    if ($runningGui.HasExited) {
        $guiClosed = $true
        break
    }
}
Assert-True $guiClosed 'silent upgrade closed the running GUI'
$infoAfterUpgrade = Get-Content -LiteralPath $infoPath -Raw
Assert-True ($infoAfterUpgrade -match ('Version: ' + [regex]::Escape($ExpectedVersion))) 'upgrade refreshed install-info.txt'
$propsAfterUpgrade = Get-ItemProperty -Path $uninstallRegKey -ErrorAction SilentlyContinue
Assert-True (($null -ne $propsAfterUpgrade) -and ($propsAfterUpgrade.DisplayVersion -eq $ExpectedVersion)) 'upgrade kept a single correct uninstall entry'

# --- 3. Installed GUI smoke test -------------------------------------------
Write-Host '==> Installed GUI smoke test (QT_QPA_PLATFORM=offscreen --smoke-test)'
$gui = Start-Process -FilePath (Join-Path $installDir 'auroku-gui.exe') -ArgumentList '--smoke-test' -Wait -PassThru
Assert-True ($gui.ExitCode -eq 0) ('installed GUI --smoke-test exit code 0 (got ' + $gui.ExitCode + ')')
Assert-True (Test-Path -LiteralPath (Join-Path $smokeStateDir 'gui.log')) 'installed GUI wrote gui.log'
Remove-Item Env:AUROKU_STATE_DIR -ErrorAction SilentlyContinue

# --- 4. Silent uninstall ----------------------------------------------------
Write-Host '==> Silent uninstall (/S)'
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
Set-Content -LiteralPath $marker -Value 'installer smoke marker - must survive uninstallation' -Encoding ascii
$uninstaller = Join-Path $installDir 'uninstall.exe'
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru
$uninstall.WaitForExit(120000) | Out-Null

Assert-True (Wait-ForPathRemoved -Path $installDir) 'install directory removed'
Assert-True (Wait-ForPathRemoved -Path $startMenuDir -TimeoutSeconds 30) 'start menu folder removed'
Assert-True (-not (Test-Path -LiteralPath $desktopLnk)) 'desktop shortcut removed'
Assert-True (-not (Test-Path -LiteralPath $uninstallRegKey)) 'HKCU uninstall entry removed'
Assert-True (-not (Test-Path -LiteralPath $appRegKey)) 'HKCU Software\auRokku removed'
Assert-True (Test-Path -LiteralPath $marker) 'user data directory preserved (marker file)'

$newLogs = @(Get-ChildItem -Path $env:TEMP -Filter 'auroku-setup-*.log' -ErrorAction SilentlyContinue |
    Where-Object { $logsBefore -notcontains $_.FullName })
Assert-True ($newLogs.Count -ge 2) ('install and uninstall logs written to %TEMP% (found ' + $newLogs.Count + ')')
$installLog = @($newLogs | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw) -match 'install: ok' })
$uninstallLog = @($newLogs | Where-Object { (Get-Content -LiteralPath $_.FullName -Raw) -match 'uninstall: ok' })
Assert-True ($installLog.Count -ge 1) 'install log contains "install: ok"'
Assert-True ($uninstallLog.Count -ge 1) 'uninstall log contains "uninstall: ok"'

# --- 5. Optional /DESKTOP + /PURGEDATA cycle -------------------------------
if ($TestPurge) {
    Write-Host '==> Optional /DESKTOP + /PURGEDATA cycle'
    $dataBackup = $null
    if (Test-Path -LiteralPath $dataDir) {
        $dataBackup = Join-Path $env:TEMP ('auroku-data-backup-' + [guid]::NewGuid().ToString('N'))
        Copy-Item -LiteralPath $dataDir -Destination $dataBackup -Recurse -Force
    }
    try {
        # /D= must stay the last parameter, so /DESKTOP goes before it.
        Start-SilentProcess -FilePath $setup -Arguments ('/S /DESKTOP /D=' + $installDir) -Label '/DESKTOP silent install' | Out-Null
        Assert-True (Test-Path -LiteralPath $desktopLnk) '/DESKTOP created the desktop shortcut'
        $purge = Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList '/S /PURGEDATA' -PassThru
        $purge.WaitForExit(120000) | Out-Null
        Assert-True (Wait-ForPathRemoved -Path $installDir) '/PURGEDATA uninstall removed the install directory'
        Assert-True (-not (Test-Path -LiteralPath $desktopLnk)) '/PURGEDATA uninstall removed the desktop shortcut'
        Assert-True (-not (Test-Path -LiteralPath $dataDir)) '/PURGEDATA removed the user data directory'
    } finally {
        if ($dataBackup) {
            Remove-Item -LiteralPath $dataDir -Recurse -Force -ErrorAction SilentlyContinue
            Copy-Item -LiteralPath $dataBackup -Destination $dataDir -Recurse -Force -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath $dataBackup -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# --- Cleanup ----------------------------------------------------------------
Remove-TestArtifacts
if (Test-Path -LiteralPath $dataDir) {
    $remaining = @(Get-ChildItem -LiteralPath $dataDir -Force -ErrorAction SilentlyContinue)
    if ($remaining.Count -eq 0) {
        Remove-Item -LiteralPath $dataDir -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ''
if ($failures.Count -gt 0) {
    Write-Host ('INSTALLER SMOKE TEST FAILED: ' + $failures.Count + ' check(s)')
    foreach ($failure in $failures) {
        Write-Host ('  - ' + $failure)
    }
    throw ('installer smoke test failed: ' + $failures.Count + ' check(s)')
}
Write-Host 'INSTALLER SMOKE TEST PASSED'
