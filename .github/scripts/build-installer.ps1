<#
.SYNOPSIS
    Build the per-user NSIS installer for the auRokku Qt 6 GUI (Windows).

.DESCRIPTION
    Wraps gui/installer/auroku.nsi with makensis. The payload is the complete
    dist/auroku-gui-windows-<arch> directory produced by build-windows.ps1:
    the GUI (auroku-gui.exe), the auroku.exe elevation helper, the Qt 6
    runtime/plugins, the VC++ runtime (MSVC builds) and the license files.

    The script synthesises the license page text (repo LICENSE + payload
    LICENSE-LGPL-3.0.txt + gui/installer/THIRD-PARTY-NOTICES.txt), derives the
    4-part numeric version required by VIProductVersion, computes the payload
    SHA256 (sorted relative-path manifest) and the Add/Remove Programs size,
    then compiles the installer and writes a .sha256 next to it.

.EXAMPLE
    .\.github\scripts\build-installer.ps1 -Version 0.3.2-qt6 -PayloadDir dist\auroku-gui-windows-x86_64 -OutFile dist\auroku-windows-x86_64-v0.3.2-qt6-setup.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [string]$PayloadDir,

    [string]$OutFile = '',
    [string]$Arch = 'x86_64',
    [string]$Makensis = ''
)

$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$repoRoot = (Get-Location).Path

function Resolve-MakensisPath {
    param([string]$Explicit)

    if ($Explicit) {
        if (-not (Test-Path $Explicit)) {
            throw "makensis not found: $Explicit"
        }
        return (Resolve-Path $Explicit).Path
    }

    $command = Get-Command makensis -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    $programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
    foreach ($candidate in @(
        (Join-Path $programFilesX86 'NSIS\makensis.exe'),
        (Join-Path 'C:\Program Files\NSIS' 'makensis.exe')
    )) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }
    throw 'makensis not found. Install NSIS 3 (winget/choco install nsis) or pass -Makensis.'
}

function Get-NumericVersion {
    param([string]$DisplayVersion)

    $value = $DisplayVersion.Trim()
    if ($value.StartsWith('v') -or $value.StartsWith('V')) {
        $value = $value.Substring(1)
    }
    # Drop prerelease/build metadata (0.3.2-qt6 -> 0.3.2).
    $base = ($value -split '[-+]')[0]
    $parts = @($base -split '\.')
    if ($parts.Count -lt 1 -or $parts.Count -gt 4) {
        throw "cannot derive a numeric version from '$DisplayVersion'"
    }
    $numeric = @()
    foreach ($part in $parts) {
        if ($part -notmatch '^\d+$') {
            throw "cannot derive a numeric version from '$DisplayVersion' (bad segment '$part')"
        }
        $number = [int]$part
        if ($number -lt 0 -or $number -gt 65535) {
            throw "version segment out of range in '$DisplayVersion': $part"
        }
        $numeric += $number
    }
    while ($numeric.Count -lt 4) {
        $numeric += 0
    }
    return ($numeric -join '.')
}

function Get-PayloadHash {
    param([string]$Root, [object[]]$Files)

    $lines = @()
    foreach ($file in ($Files | Sort-Object FullName)) {
        $relative = $file.FullName.Substring($Root.Length).TrimStart('\').Replace('\', '/')
        $hash = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $lines += ($relative + '|' + $file.Length + '|' + $hash)
    }
    $manifest = ($lines -join [Environment]::NewLine)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($manifest))
    } finally {
        $sha.Dispose()
    }
    return (-join ($digest | ForEach-Object { $_.ToString('x2') }))
}

# --- Resolve inputs ---------------------------------------------------------
$makensisPath = Resolve-MakensisPath -Explicit $Makensis
$nsisRoot = Split-Path $makensisPath -Parent
if (-not $env:NSISDIR -and (Test-Path (Join-Path $nsisRoot 'Include\MUI2.nsh'))) {
    $env:NSISDIR = $nsisRoot
}

$payloadFull = (Resolve-Path $PayloadDir).Path
foreach ($required in @('auroku-gui.exe', 'auroku.exe', 'LICENSE')) {
    if (-not (Test-Path (Join-Path $payloadFull $required))) {
        throw "payload is missing $required - run .github/scripts/build-windows.ps1 first: $payloadFull"
    }
}

$version4 = Get-NumericVersion -DisplayVersion $Version
$buildTime = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')

$scriptPath = Join-Path $repoRoot 'gui\installer\auroku.nsi'
if (-not (Test-Path $scriptPath)) {
    throw "installer script not found: $scriptPath"
}

if (-not $OutFile) {
    $suffix = ''
    if ($env:AUROKU_BUILD_DEBUG -eq '1') {
        $suffix = '-debug'
    }
    $OutFile = Join-Path (Join-Path $repoRoot 'dist') ("auroku-windows-$Arch-v$Version$suffix-setup.exe")
}
if (-not [System.IO.Path]::IsPathRooted($OutFile)) {
    $OutFile = Join-Path $repoRoot $OutFile
}
$outFull = [System.IO.Path]::GetFullPath($OutFile)
$outDir = Split-Path $outFull -Parent
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

# --- Payload statistics -----------------------------------------------------
$payloadFiles = @(Get-ChildItem $payloadFull -Recurse -File)
if ($payloadFiles.Count -eq 0) {
    throw "payload directory is empty: $payloadFull"
}
$payloadBytes = ($payloadFiles | Measure-Object -Property Length -Sum).Sum
$estimatedSizeKb = [int][math]::Ceiling($payloadBytes / 1KB)
if ($estimatedSizeKb -lt 1) {
    $estimatedSizeKb = 1
}
$payloadHash = Get-PayloadHash -Root $payloadFull -Files $payloadFiles

# --- Combined license text for MUI_PAGE_LICENSE ----------------------------
$repoLicense = Join-Path $repoRoot 'LICENSE'
if (-not (Test-Path $repoLicense)) {
    throw "repository LICENSE not found: $repoLicense"
}
$repoLicenseText = Get-Content -Path $repoLicense -Raw

# The repository LICENSE is authoritative; only the heading is derived from it
# so the installer never labels the bundled text incorrectly.
$projectLicenseName = 'auRokku license terms'
if (($repoLicenseText -match 'GNU GENERAL PUBLIC LICENSE') -and ($repoLicenseText -match 'Version 3')) {
    $projectLicenseName = 'GNU General Public License, version 3 (auRokku)'
} elseif ($repoLicenseText -match 'MIT License') {
    $projectLicenseName = 'MIT License (auRokku)'
}

$licenseParts = New-Object System.Collections.Generic.List[string]
foreach ($line in @(
    'auRokku (Qt6) - license texts',
    '=============================',
    '',
    'auRokku is distributed under the terms of the repository LICENSE below.',
    'That text is followed by the license of libwdi (statically linked in',
    'Windows release builds) and the third-party notices of the Qt 6 runtime.',
    '',
    '--------------------------------------------------------------------------',
    $projectLicenseName,
    '--------------------------------------------------------------------------',
    ''
)) {
    $licenseParts.Add($line)
}

foreach ($line in (Get-Content -Path $repoLicense)) {
    $licenseParts.Add($line)
}

$payloadLgpl = Join-Path $payloadFull 'LICENSE-LGPL-3.0.txt'
if (Test-Path $payloadLgpl) {
    foreach ($line in @(
        '',
        '',
        '--------------------------------------------------------------------------',
        'GNU Lesser General Public License, version 3 (libwdi)',
        '--------------------------------------------------------------------------',
        ''
    )) {
        $licenseParts.Add($line)
    }
    foreach ($line in (Get-Content -Path $payloadLgpl)) {
        $licenseParts.Add($line)
    }
} else {
    foreach ($line in @(
        '',
        '',
        'This build does not link libwdi statically, so no LGPLv3 text is',
        'included. Release builds ship LICENSE-LGPL-3.0.txt.'
    )) {
        $licenseParts.Add($line)
    }
}

$noticesPath = Join-Path $repoRoot 'gui\installer\THIRD-PARTY-NOTICES.txt'
if (Test-Path $noticesPath) {
    foreach ($line in @(
        '',
        '',
        '--------------------------------------------------------------------------',
        'Third-party notices',
        '--------------------------------------------------------------------------',
        ''
    )) {
        $licenseParts.Add($line)
    }
    foreach ($line in (Get-Content -Path $noticesPath)) {
        $licenseParts.Add($line)
    }
} else {
    Write-Warning 'gui/installer/THIRD-PARTY-NOTICES.txt is missing; the license page omits the third-party notices.'
}

$nonAscii = @($licenseParts | Where-Object { ($_.ToCharArray() | Where-Object { [int]$_ -gt 127 }) })
if ($nonAscii.Count -gt 0) {
    throw 'the license text must stay ASCII for the NSIS license page (it is read with the ANSI code page)'
}

$licenseTemp = Join-Path $env:TEMP ('auroku-installer-license-' + [guid]::NewGuid().ToString('N') + '.txt')
[System.IO.File]::WriteAllText($licenseTemp, ($licenseParts -join [Environment]::NewLine), (New-Object System.Text.ASCIIEncoding))

# --- Compile ----------------------------------------------------------------
Write-Host '==> Building NSIS installer'
Write-Host ("    version:    $Version (numeric $version4)")
Write-Host ("    payload:    $payloadFull")
Write-Host ("    files:      $($payloadFiles.Count) ($([math]::Round($payloadBytes / 1MB, 2)) MB, $estimatedSizeKb KiB estimated size)")
Write-Host ("    payload sha256: $payloadHash")
Write-Host ("    output:     $outFull")

$arguments = @(
    '/V2',
    ('/DAPP_VERSION=' + $Version),
    ('/DAPP_VERSION_4=' + $version4),
    ('/DPAYLOAD_DIR=' + $payloadFull),
    ('/DOUT_FILE=' + $outFull),
    ('/DESTIMATED_SIZE_KB=' + $estimatedSizeKb),
    ('/DLICENSE_FILE=' + $licenseTemp),
    ('/DBUILD_ARCH=' + $Arch),
    ('/DBUILD_TIME=' + $buildTime),
    ('/DPAYLOAD_SHA256=' + $payloadHash),
    $scriptPath
)

try {
    & $makensisPath @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "makensis failed with exit code $LASTEXITCODE"
    }
} finally {
    Remove-Item -LiteralPath $licenseTemp -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path $outFull)) {
    throw "the installer was not produced: $outFull"
}

$outItem = Get-Item $outFull
$outHash = (Get-FileHash -Path $outFull -Algorithm SHA256).Hash.ToLowerInvariant()
$shaFile = $outFull + '.sha256'
Set-Content -Path $shaFile -Value ($outHash + '  ' + $outItem.Name) -Encoding ascii

Write-Host ''
Write-Host ("Installer:  " + $outItem.FullName)
Write-Host ("Size:       " + [math]::Round($outItem.Length / 1MB, 2) + ' MB (' + $outItem.Length + ' bytes)')
Write-Host ("SHA256:     " + $outHash)
Write-Host ("Checksums:  " + $shaFile)
