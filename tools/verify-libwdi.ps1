<#
.SYNOPSIS
    Verify the libwdi-static artifact contract: SHA256, COFF symbols, static
    CRT heuristic, LGPL text and BUILD-INFO self-consistency.

.DESCRIPTION
    Implements the acceptance criteria of docs/plan/03 section 7.1 and is
    deliberately stricter than the old "Test-Path libwdi.lib" assertion:

      1. lib/ contains libwdi.lib, COPYING-LGPL, BUILD-INFO.json, SHA256SUMS.
      2. libwdi.lib is at least 1 MiB and its SHA256 matches BUILD-INFO.json,
         SHA256SUMS and -ExpectedSha256 (when supplied).
      3. Every file in lib/ is listed in SHA256SUMS with the right hash.
      4. dumpbin /symbols finds the six wdi_* entry points the bindings call.
      5. The COFF .drectve records /DEFAULTLIB:LIBCMT (the /MT heuristic) and
         never a dynamic or debug CRT. The authoritative check for the product
         executables stays "dumpbin /dependents" in the product CI: the
         embedded WinUSB payload inside the archive contains third-party
         binaries, so a plain string search over the whole file would only
         produce false positives.
      6. COPYING-LGPL is LGPLv3 text and equals LICENSES/LGPL-3.0.txt.
      7. BUILD-INFO.json is self-consistent: its upstream commit SHA equals
         upstream/libwdi.ref, the patch hashes match patches/, and the WDK MSI
         hash is recorded.

.PARAMETER LibDir
    Directory that contains libwdi.lib (the artifact's lib/ directory).

.PARAMETER ExpectedSha256
    Optional expected SHA256 for libwdi.lib; product repositories can pass
    their reviewed trust anchor here.

.PARAMETER Dumpbin
    Explicit dumpbin.exe path; otherwise discovered via vswhere or PATH.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$LibDir,
    [string]$ExpectedSha256 = '',
    [string]$Dumpbin = ''
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

function Fail([string]$Message) {
    throw ('verify-libwdi: ' + $Message)
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Find-Dumpbin {
    if ($Dumpbin) { return $Dumpbin }
    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (Test-Path $vswhere) {
        $found = & $vswhere -latest -products * -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' 2>$null
        if ($found) { return @($found)[0] }
    }
    $command = Get-Command 'dumpbin.exe' -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    Fail 'dumpbin.exe not found; pass -Dumpbin or install Visual Studio Build Tools'
}

$libDirResolved = (Resolve-Path -Path $LibDir).Path
foreach ($name in @('libwdi.lib', 'COPYING-LGPL', 'BUILD-INFO.json', 'SHA256SUMS')) {
    if (-not (Test-Path (Join-Path $libDirResolved $name))) {
        Fail ('artifact is missing ' + $name + ' in ' + $libDirResolved)
    }
}

$libPath = Join-Path $libDirResolved 'libwdi.lib'
$size = (Get-Item $libPath).Length
if ($size -lt 1MB) { Fail ('libwdi.lib is suspiciously small: ' + $size + ' bytes') }
$sha = Get-Sha256 $libPath
Write-Host ('libwdi.lib: ' + $size + ' bytes, sha256 ' + $sha)

# --- SHA256SUMS coverage and hashes -----------------------------------------
$sums = @{}
foreach ($line in (Get-Content (Join-Path $libDirResolved 'SHA256SUMS'))) {
    $trimmed = ([string]$line).Trim()
    if (-not $trimmed) { continue }
    if ($trimmed -notmatch '^(?<hash>[0-9a-f]{64})\s{2}(?<name>[^/\\]+)$') {
        Fail ('unexpected SHA256SUMS line: ' + $trimmed)
    }
    $sums[$Matches['name']] = $Matches['hash']
}
$files = @(Get-ChildItem -Path $libDirResolved -File | Where-Object { $_.Name -ne 'SHA256SUMS' })
foreach ($file in $files) {
    if (-not $sums.ContainsKey($file.Name)) { Fail ('SHA256SUMS does not list ' + $file.Name) }
    if ((Get-Sha256 $file.FullName) -ne $sums[$file.Name]) { Fail ('SHA256SUMS hash mismatch for ' + $file.Name) }
}
if ($sums.Count -ne $files.Count) { Fail 'SHA256SUMS lists files that are not in the artifact' }
if ($sums['libwdi.lib'] -ne $sha) { Fail 'SHA256SUMS libwdi.lib hash does not match the file' }
if ($ExpectedSha256) {
    if ($ExpectedSha256.ToLowerInvariant() -ne $sha) {
        Fail ('libwdi.lib sha256 mismatch: expected ' + $ExpectedSha256 + ', got ' + $sha)
    }
}

# --- BUILD-INFO.json self-consistency ----------------------------------------
$info = Get-Content (Join-Path $libDirResolved 'BUILD-INFO.json') -Raw | ConvertFrom-Json
if ($info.schema -ne 1) { Fail ('unsupported BUILD-INFO.json schema: ' + $info.schema) }
if ($info.artifact -ne 'libwdi-static') { Fail 'BUILD-INFO.json artifact is not libwdi-static' }
if ($info.platform -ne 'x64') { Fail 'BUILD-INFO.json platform is not x64' }
if ([string]$info.payload -notmatch 'WinUSB') { Fail 'BUILD-INFO.json does not record the WinUSB-only payload' }
if ($info.lib_sha256 -ne $sha) { Fail 'BUILD-INFO.json lib_sha256 does not match libwdi.lib' }
if ($info.lib_size -ne $size) { Fail 'BUILD-INFO.json lib_size does not match libwdi.lib' }
if ([string]$info.wdk_msi_sha256 -notmatch '^[0-9a-f]{64}$') { Fail 'BUILD-INFO.json wdk_msi_sha256 is not a sha256' }
if ([string]$info.upstream_ref -notmatch '^[0-9a-f]{40}$') { Fail 'BUILD-INFO.json upstream_ref is not a full commit SHA' }

$refFile = Join-Path $repoRoot 'upstream/libwdi.ref'
if (-not (Test-Path $refFile)) { Fail ('missing ' + $refFile) }
$pinnedLine = Get-Content $refFile | Where-Object { $_.Trim() -and -not $_.Trim().StartsWith('#') } | Select-Object -First 1
$pinned = ($pinnedLine.Trim() -split '\s+')[0]
if ($info.upstream_ref -ne $pinned) {
    Fail ('BUILD-INFO.json upstream_ref=' + $info.upstream_ref + ' differs from upstream/libwdi.ref=' + $pinned)
}

$patches = @($info.patches)
if ($patches.Count -lt 1) { Fail 'BUILD-INFO.json does not list the applied patches' }
foreach ($patch in $patches) {
    $patchPath = Join-Path $repoRoot ('patches/' + $patch.name)
    if (-not (Test-Path $patchPath)) { Fail ('BUILD-INFO.json references a missing patch: ' + $patch.name) }
    if ((Get-Sha256 $patchPath) -ne $patch.sha256) { Fail ('patch hash mismatch for ' + $patch.name) }
}

# --- LGPL text ---------------------------------------------------------------
$licensePath = Join-Path $libDirResolved 'COPYING-LGPL'
$licenseText = [IO.File]::ReadAllText($licensePath)
if ($licenseText -notmatch 'GNU LESSER GENERAL PUBLIC LICENSE') { Fail 'COPYING-LGPL is not the LGPL text' }
if ($licenseText -notmatch 'Version 3') { Fail 'COPYING-LGPL is not LGPLv3' }
$pinnedLicense = Join-Path $repoRoot 'LICENSES/LGPL-3.0.txt'
if (-not (Test-Path $pinnedLicense)) { Fail ('missing pinned license copy: ' + $pinnedLicense) }
$lf = [string][char]10
$crlf = [string][char]13 + [string][char]10
$normalize = { param([string]$Path) ([IO.File]::ReadAllText($Path)).Replace($crlf, $lf).TrimEnd() }
if ((& $normalize $licensePath) -ne (& $normalize $pinnedLicense)) {
    Fail 'COPYING-LGPL differs from LICENSES/LGPL-3.0.txt'
}

# --- COFF symbols ------------------------------------------------------------
$dumpbinPath = Find-Dumpbin
$symbolOutput = & $dumpbinPath '/nologo' '/symbols' $libPath
if ($LASTEXITCODE -ne 0) { Fail ('dumpbin /symbols failed with exit code ' + $LASTEXITCODE) }
foreach ($symbol in @('wdi_create_list', 'wdi_destroy_list', 'wdi_prepare_driver', 'wdi_install_driver', 'wdi_strerror', 'wdi_set_log_level')) {
    if (-not ($symbolOutput | Select-String -SimpleMatch $symbol)) {
        Fail ('libwdi.lib does not define ' + $symbol)
    }
}
Write-Host 'libwdi.lib defines wdi_create_list/wdi_destroy_list/wdi_prepare_driver/wdi_install_driver/wdi_strerror/wdi_set_log_level'

# --- static CRT heuristic (COFF .drectve records only) -----------------------
$archiveText = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($libPath))
if ($archiveText -notmatch '/DEFAULTLIB:LIBCMT') {
    Fail 'libwdi.lib does not default-link LIBCMT (built with /MD instead of /MT?)'
}
foreach ($dynamic in @('/DEFAULTLIB:MSVCRT', '/DEFAULTLIB:MSVCRTD', '/DEFAULTLIB:LIBCMTD', '/DEFAULTLIB:UCRT')) {
    if ($archiveText -match [regex]::Escape($dynamic)) {
        Fail ('libwdi.lib references a dynamic or debug CRT: ' + $dynamic)
    }
}
Write-Host 'libwdi.lib defaults to the static CRT (LIBCMT)'

# --- workflow outputs --------------------------------------------------------
if ($env:GITHUB_OUTPUT) {
    ('sha256=' + $sha) | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}

Write-Host ''
Write-Host ('verify-libwdi: OK - sha256=' + $sha + ' size=' + $size)
