<#
.SYNOPSIS
    Disable or restore the Windows usbser.sys kernel service (testing only).

.DESCRIPTION
    Sets
      HKLM\SYSTEM\CurrentControlSet\Services\usbser\Start = 4
    (SERVICE_DISABLED) so Windows no longer binds the Microsoft USB Serial
    driver to CDC devices.

    The previous Start value is saved to
      %ProgramData%\auroku\usbser-service.json
    so "-Action Restore" can put it back.  Disable and Restore require an
    elevated PowerShell session.

    This is a development/troubleshooting helper.  It affects EVERY CDC serial
    device on the machine (normal COM ports stop working), it does not install
    WinUSB, and it does not by itself make libusb able to open the device.

.PARAMETER Action
    Status (default), Disable or Restore.

.PARAMETER FallbackStart
    Start value used by Restore when no backup file exists (default 3 =
    DEMAND_START, the Windows default).  Restore refuses to use the fallback
    unless -Force is also specified.

.PARAMETER Force
    Allow Restore without a backup (uses -FallbackStart).

.EXAMPLE
    .\usbser-service.ps1 -Action Status

.EXAMPLE
    .\usbser-service.ps1 -Action Disable

.EXAMPLE
    .\usbser-service.ps1 -Action Restore

.NOTES
    Companion registry file: disable-usbser.reg
    Documentation: docs/windows-disable-usbser.md
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [ValidateSet('Status', 'Disable', 'Restore')]
    [string]$Action = 'Status',

    [ValidateSet(2, 3, 4)]
    [int]$FallbackStart = 3,

    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ServiceName = 'usbser'
$ServiceKey = "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName"
$StateDir = Join-Path $env:ProgramData 'auroku'
$StatePath = Join-Path $StateDir 'usbser-service.json'

$StartNames = @{
    0 = 'BOOT_START'
    1 = 'SYSTEM_START'
    2 = 'AUTO_START'
    3 = 'DEMAND_START'
    4 = 'DISABLED'
}

function Assert-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This action requires an elevated PowerShell session (Run as Administrator).'
    }
}

function Get-UsbSerStart {
    if (-not (Test-Path -Path $ServiceKey)) {
        throw "Registry key not found: $ServiceKey"
    }
    [int](Get-ItemProperty -Path $ServiceKey -Name Start).Start
}

function Get-StartName {
    param([int]$Value)
    if ($StartNames.ContainsKey($Value)) {
        return $StartNames[$Value]
    }
    return "UNKNOWN($Value)"
}

function Read-Backup {
    if (-not (Test-Path -Path $StatePath)) {
        return $null
    }
    try {
        return (Get-Content -Raw -Path $StatePath | ConvertFrom-Json)
    } catch {
        Write-Warning "Cannot parse backup file $StatePath : $($_.Exception.Message)"
        return $null
    }
}

function Write-Backup {
    param([int]$StartValue)
    if (-not (Test-Path -Path $StateDir)) {
        New-Item -ItemType Directory -Path $StateDir -Force | Out-Null
    }
    [pscustomobject]@{
        Service = $ServiceName
        Start   = $StartValue
        SavedAt = (Get-Date).ToString('o')
        SavedBy = $env:USERNAME
    } | ConvertTo-Json | Set-Content -Path $StatePath -Encoding ascii
}

function Set-UsbSerStart {
    param([int]$StartValue)
    Set-ItemProperty -Path $ServiceKey -Name Start -Value $StartValue -Type DWord
}

function Show-Status {
    $current = Get-UsbSerStart
    $backup = Read-Backup

    Write-Host "Service : $ServiceName"
    Write-Host "Registry: $ServiceKey"
    Write-Host ("Start   : {0} ({1})" -f $current, (Get-StartName $current))
    if ($null -ne $backup) {
        Write-Host ("Backup  : Start={0} saved {1} by {2}" -f $backup.Start, $backup.SavedAt, $backup.SavedBy)
    } else {
        Write-Host 'Backup  : none'
    }
    Write-Host ''

    if (Get-Command sc.exe -ErrorAction SilentlyContinue) {
        & sc.exe qc $ServiceName 2>&1 | ForEach-Object { Write-Host "  $_" }
    }
}

function Disable-UsbSer {
    Assert-Administrator
    $current = Get-UsbSerStart

    if ($current -eq 4) {
        Write-Host 'usbser is already disabled (Start=4).'
        return
    }

    if ($null -eq (Read-Backup)) {
        if ($PSCmdlet.ShouldProcess($StatePath, "save usbser Start=$current")) {
            Write-Backup -StartValue $current
            Write-Host ("Saved previous Start={0} to {1}" -f $current, $StatePath)
        }
    }

    if ($PSCmdlet.ShouldProcess($ServiceKey, 'set usbser Start=4 (SERVICE_DISABLED)')) {
        Set-UsbSerStart -StartValue 4
        # Stop the running driver instance if it is loaded; ignore "not started".
        & sc.exe stop $ServiceName 2>&1 | Out-Null
        Write-Host 'usbser.sys is now disabled (Start=4).'
        Write-Host 'Reboot Windows or unplug/replug the USB device for the change to take effect.'
        Write-Host ("Restore with: {0} -Action Restore" -f $PSCommandPath)
    }
}

function Restore-UsbSer {
    Assert-Administrator
    $backup = Read-Backup
    if ($null -eq $backup) {
        if (-not $Force) {
            $message = "No backup found at {0}. If you disabled usbser outside this script, " +
                       "re-run with -Force -FallbackStart 3 (or the value you want)."
            throw ($message -f $StatePath)
        }
        $target = $FallbackStart
        Write-Warning ("No backup found; restoring to FallbackStart={0} because -Force was given." -f $target)
    } else {
        $target = [int]$backup.Start
    }

    if ($target -eq 4) {
        throw ("Backup/fallback value is 4 (DISABLED); refusing to restore an already-disabled value. " +
               "Use -Force -FallbackStart 3 to override.")
    }

    if ($PSCmdlet.ShouldProcess($ServiceKey, "set usbser Start=$target")) {
        Set-UsbSerStart -StartValue $target
        Remove-Item -Path $StatePath -Force -ErrorAction SilentlyContinue
        Write-Host ("usbser restored to Start={0} ({1})." -f $target, (Get-StartName $target))
        Write-Host 'Reboot Windows or unplug/replug the USB device for the change to take effect.'
    }
}

switch ($Action) {
    'Status' { Show-Status }
    'Disable' { Disable-UsbSer }
    'Restore' { Restore-UsbSer }
}
