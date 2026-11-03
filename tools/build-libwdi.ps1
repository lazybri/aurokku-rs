<#
.SYNOPSIS
    Build the static libwdi 1.5.1 library (x64 only, WinUSB payload, static CRT).

.DESCRIPTION
    Single source of truth for producing libwdi.lib. Ported 1:1 from the inline
    PowerShell step that used to live in the product repository's
    .github/workflows/build.yml (main @ b219bd5, lines 111-157) and fixes the
    gaps documented in docs/plan/03 section 1.5:

      D1  upstream is pinned to a full commit SHA (upstream/libwdi.ref), never a
          mutable tag; the workflow input is cross-checked against the pin.
      D2  config.h / vcxproj edits are real patch files in patches/, applied
          with "git apply --check" first (no inline -replace: diffable, and it
          fails loudly when upstream moves).
      D3  the artifact always contains COPYING-LGPL (no silent skip).
      D7  the WDK redistributable MSI is SHA256-verified and cached.

    Pipeline (mirrors the old inline step):
      1. WDK redist MSI -> SHA256 verify -> msiexec /a extract (no install).
      2. git fetch + checkout the pinned libwdi commit; rev-parse assertion.
      3. Normalise msvc/config.h and libwdi/.msvc/libwdi_static.vcxproj to LF,
         apply patches/*.patch, then assert the patched content.
      4. msbuild detect_64build(x64) -> installer_x64(x64) -> embedder(Win32)
         -> libwdi_static(x64), with the same BuildMacros/SolutionDir as before.
      5. Stage the artifact contract under <OutDir>/lib/:
           libwdi.lib  COPYING-LGPL  BUILD-INFO.json  SHA256SUMS

.PARAMETER LibwdiSha
    Upstream pbatard/libwdi commit SHA. Defaults to the value in
    upstream/libwdi.ref; when supplied (the reusable workflow passes
    libwdi-sha) it must match that file.

.PARAMETER WdkMsiUrl
    WDK redistributable download URL (Microsoft redirect).

.PARAMETER WdkMsiSha256
    Expected SHA256 of that MSI. If Microsoft rotates the download the build
    fails here instead of silently producing a different library.

.PARAMETER WorkDir
    Scratch directory, default <repo>/.work/libwdi.

.PARAMETER OutDir
    Artifact root, default <repo>/libwdi-out (the artifact itself contains
    lib/...).

.EXAMPLE
    ./tools/build-libwdi.ps1

.NOTES
    Requires Windows, Visual Studio Build Tools (msbuild + MSVC v143) and
    network access to Microsoft and GitHub. Product repositories never run
    this script: they download the artifact produced by
    .github/workflows/libwdi-static.yml at an immutable tag.
#>
[CmdletBinding()]
param(
    [string]$LibwdiSha = '',
    [string]$OutDir = '',
    [string]$WorkDir = '',
    [string]$WdkMsiUrl = 'https://go.microsoft.com/fwlink/p/?LinkID=253170',
    [string]$WdkMsiSha256 = '29314207814ce9d5d73695f7e9239539cf37c79e750b9d5ea5a5ef5487a583d6'
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutDir) { $OutDir = Join-Path $repoRoot 'libwdi-out' }
if (-not $WorkDir) { $WorkDir = Join-Path $repoRoot '.work/libwdi' }
$cacheDir = Join-Path $repoRoot '.cache'
$lf = [string][char]10
$crlf = [string][char]13 + [string][char]10
$upstreamUrl = 'https://github.com/pbatard/libwdi.git'

function Write-Step([string]$Message) {
    Write-Host ''
    Write-Host ('==> ' + $Message)
}

function Invoke-Native {
    param([string]$File, [string[]]$Arguments, [string]$What)
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw ($What + ' failed with exit code ' + $LASTEXITCODE)
    }
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Normalize-Lf([string]$Path) {
    # Keep a UTF-8 BOM when the upstream file has one (libwdi_static.vcxproj
    # does): git apply compares the first line, BOM included.
    $bytes = [IO.File]::ReadAllBytes($Path)
    $hasBom = ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)
    $offset = if ($hasBom) { 3 } else { 0 }
    $text = [Text.Encoding]::UTF8.GetString($bytes, $offset, $bytes.Length - $offset)
    $text = $text.Replace($crlf, $lf)
    [IO.File]::WriteAllText($Path, $text, (New-Object System.Text.UTF8Encoding($hasBom)))
}

function Get-PinnedUpstreamSha {
    $refFile = Join-Path $repoRoot 'upstream/libwdi.ref'
    if (-not (Test-Path $refFile)) { throw ('missing upstream reference file: ' + $refFile) }
    $line = Get-Content $refFile | Where-Object { $_.Trim() -and -not $_.Trim().StartsWith('#') } | Select-Object -First 1
    if (-not $line) { throw ('upstream/libwdi.ref has no commit SHA: ' + $refFile) }
    $sha = ($line.Trim() -split '\s+')[0]
    if ($sha -notmatch '^[0-9a-f]{40}$') {
        throw ('upstream/libwdi.ref must contain a full 40-hex commit SHA, got: ' + $sha)
    }
    return $sha
}

# ---------------------------------------------------------------------------
# 0) Resolve and cross-check the pinned upstream commit
# ---------------------------------------------------------------------------
$pinned = Get-PinnedUpstreamSha
if ($LibwdiSha) {
    if ($LibwdiSha -notmatch '^[0-9a-f]{40}$') {
        throw ('libwdi-sha must be a full 40-hex commit SHA, got: ' + $LibwdiSha)
    }
    if ($LibwdiSha -ne $pinned) {
        throw ('workflow input libwdi-sha=' + $LibwdiSha + ' does not match upstream/libwdi.ref=' + $pinned + '. Update the pin in a dedicated commit and publish a new libwdi-<upstream>-x64-r<N> tag; tags are immutable.')
    }
}
$LibwdiSha = $pinned
Write-Host ('Pinned upstream libwdi commit: ' + $LibwdiSha)

New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null

# ---------------------------------------------------------------------------
# 1) WDK redist MSI: cache -> SHA256 verify -> msiexec /a extract
# ---------------------------------------------------------------------------
Write-Step 'Preparing the WDK redistributable (WinUSB payload)'
$msi = Join-Path $cacheDir 'wdk-redist.msi'
if (-not (Test-Path $msi)) {
    Write-Host ('    downloading ' + $WdkMsiUrl)
    Invoke-Native -File 'curl.exe' -Arguments @('-sSL', $WdkMsiUrl, '-o', $msi) -What 'WDK redist download'
}
$msiSha = Get-Sha256 $msi
if ($msiSha -ne $WdkMsiSha256.ToLowerInvariant()) {
    throw ('WDK redist MSI sha256 mismatch: expected ' + $WdkMsiSha256 + ', got ' + $msiSha + '. Microsoft rotated the download; verify the new MSI, then update WdkMsiSha256 here and the cache key in .github/workflows/libwdi-static.yml in a reviewed commit.')
}
Write-Host ('    wdk-redist.msi sha256 ' + $msiSha)

$wdkRoot = Join-Path $WorkDir 'wdk'
$wdkKits = Join-Path $wdkRoot 'Windows Kits/8.0'
if (-not (Test-Path $wdkKits)) {
    Write-Host '    extracting with msiexec /a (administrative install: no admin rights needed)'
    $arguments = @('/a', ('"' + $msi + '"'), '/qn', ('TARGETDIR="' + $wdkRoot + '"'))
    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw ('msiexec /a failed with exit code ' + $process.ExitCode) }
}
if (-not (Test-Path $wdkKits)) {
    throw ('WDK extraction did not produce "Windows Kits/8.0" under ' + $wdkRoot)
}

# ---------------------------------------------------------------------------
# 2) Upstream libwdi at the pinned commit (never a tag)
# ---------------------------------------------------------------------------
Write-Step ('Cloning pbatard/libwdi at ' + $LibwdiSha)
$src = Join-Path $WorkDir 'libwdi'
if (-not (Test-Path (Join-Path $src '.git'))) {
    if (Test-Path $src) { Remove-Item $src -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $src | Out-Null
    Invoke-Native -File 'git' -Arguments @('-C', $src, 'init', '--quiet') -What 'git init'
    Invoke-Native -File 'git' -Arguments @('-C', $src, 'remote', 'add', 'origin', $upstreamUrl) -What 'git remote add'
}
# Deterministic worktree: no CRLF conversion from the host git config. The
# vcxproj still asks for CRLF through upstream .gitattributes, hence the
# explicit LF normalisation before patching below.
Invoke-Native -File 'git' -Arguments @('-C', $src, 'config', 'core.autocrlf', 'false') -What 'git config core.autocrlf'
Invoke-Native -File 'git' -Arguments @('-C', $src, 'config', 'core.eol', 'lf') -What 'git config core.eol'
$previousPrompt = $env:GIT_TERMINAL_PROMPT
$env:GIT_TERMINAL_PROMPT = '0'
try {
    Invoke-Native -File 'git' -Arguments @('-C', $src, 'fetch', '--quiet', '--depth', '1', 'origin', $LibwdiSha) -What ('git fetch ' + $LibwdiSha)
} finally {
    $env:GIT_TERMINAL_PROMPT = $previousPrompt
}
Invoke-Native -File 'git' -Arguments @('-C', $src, 'checkout', '--force', '--quiet', $LibwdiSha) -What 'git checkout'
Invoke-Native -File 'git' -Arguments @('-C', $src, 'clean', '-fdxq') -What 'git clean'
$head = (& git -C $src rev-parse HEAD).Trim()
if ($head -ne $LibwdiSha) {
    throw ('libwdi checkout mismatch: expected ' + $LibwdiSha + ', got ' + $head)
}
Write-Host ('    checked out ' + $head)

# ---------------------------------------------------------------------------
# 3) Patches: config.h (x64/WinUSB only) + drop vcxproj project references
# ---------------------------------------------------------------------------
Write-Step 'Applying patches (config.h x64/WinUSB-only, drop MSB8013 project refs)'
$configPath = Join-Path $src 'msvc/config.h'
$staticProjPath = Join-Path $src 'libwdi/.msvc/libwdi_static.vcxproj'
foreach ($target in @($configPath, $staticProjPath)) {
    if (-not (Test-Path $target)) { throw ('upstream layout changed, missing: ' + $target) }
    Normalize-Lf $target
}

$patchDir = Join-Path $repoRoot 'patches'
$patches = @(Get-ChildItem -Path $patchDir -Filter '*.patch' | Sort-Object Name)
if ($patches.Count -eq 0) { throw ('no patches found in ' + $patchDir) }
foreach ($patch in $patches) {
    Invoke-Native -File 'git' -Arguments @('-C', $src, 'apply', '--check', '--whitespace=nowarn', $patch.FullName) -What ('git apply --check ' + $patch.Name)
    Invoke-Native -File 'git' -Arguments @('-C', $src, 'apply', '--whitespace=nowarn', $patch.FullName) -What ('git apply ' + $patch.Name)
    Write-Host ('    applied ' + $patch.Name)
}

# Assert the patched sources instead of trusting git apply's exit code alone.
$configText = [IO.File]::ReadAllText($configPath)
foreach ($expected in @('/* #define OPT_M32 */', '/* #define OPT_ARM */', '/* #define LIBUSB0_DIR (not embedded) */', '/* #define LIBUSBK_DIR (not embedded) */')) {
    if (-not $configText.Contains($expected)) {
        throw ('config.h was not patched as expected; missing: ' + $expected)
    }
}
foreach ($forbidden in @('(?m)^#define OPT_M32\s*$', '(?m)^#define OPT_ARM\s*$', '(?m)^#define LIBUSB0_DIR\s', '(?m)^#define LIBUSBK_DIR\s')) {
    if ($configText -match $forbidden) {
        throw ('config.h still enables a payload that must be off: ' + $forbidden)
    }
}
$staticProjText = [IO.File]::ReadAllText($staticProjPath)
if ($staticProjText -match 'ProjectReference') {
    throw 'libwdi_static.vcxproj still contains ProjectReference entries (MSB8013 risk)'
}
try { [xml]$staticProjText | Out-Null } catch {
    throw ('patched libwdi_static.vcxproj is not valid XML: ' + $_.Exception.Message)
}

# ---------------------------------------------------------------------------
# 4) msbuild: same four targets and arguments as the old inline step
# ---------------------------------------------------------------------------
Write-Step 'Building libwdi.lib with msbuild'
$msbuildCommand = Get-Command 'msbuild' -ErrorAction SilentlyContinue
if (-not $msbuildCommand) {
    throw 'msbuild.exe not found on PATH; install Visual Studio Build Tools (MSVC v143) or run microsoft/setup-msbuild first'
}
$msbuild = $msbuildCommand.Source
$solutionDir = $src + '\'
# Forward slashes: MSVC would treat \a, \w and \W in the macro as escape
# sequences and corrupt the path.
$wdkDir = (Join-Path $wdkRoot 'Windows Kits/8.0') -replace '\\', '/'
$macros = 'WDK_DIR="' + $wdkDir + '"'
$common = @('/m', '/p:Configuration=Release', ('/p:SolutionDir=' + $solutionDir), ('/p:BuildMacros=' + $macros))
$msvcDir = Join-Path $src 'libwdi/.msvc'

# detect_64build generates libwdi/build64.h which embedder includes.
Invoke-Native -File $msbuild -Arguments (@((Join-Path $msvcDir 'detect_64build.vcxproj')) + $common + @('/p:Platform=x64')) -What 'msbuild detect_64build'
Invoke-Native -File $msbuild -Arguments (@((Join-Path $msvcDir 'installer_x64.vcxproj')) + $common + @('/p:Platform=x64')) -What 'msbuild installer_x64'
# embedder only has a Win32 configuration.
Invoke-Native -File $msbuild -Arguments (@((Join-Path $msvcDir 'embedder.vcxproj')) + $common + @('/p:Platform=Win32')) -What 'msbuild embedder'
Invoke-Native -File $msbuild -Arguments (@((Join-Path $msvcDir 'libwdi_static.vcxproj')) + $common + @('/p:Platform=x64')) -What 'msbuild libwdi_static'

$built = Join-Path $src 'x64/Release/lib/libwdi.lib'
if (-not (Test-Path $built)) { throw ('libwdi.lib was not produced: ' + $built) }

# ---------------------------------------------------------------------------
# 5) Stage the artifact contract (everything lives under lib/)
# ---------------------------------------------------------------------------
Write-Step 'Staging the artifact contract'
if (Test-Path $OutDir) { Remove-Item $OutDir -Recurse -Force }
$libDir = Join-Path $OutDir 'lib'
New-Item -ItemType Directory -Force -Path $libDir | Out-Null
$libTarget = Join-Path $libDir 'libwdi.lib'
Copy-Item $built $libTarget -Force

$copying = Join-Path $src 'COPYING-LGPL'
if (-not (Test-Path $copying)) { throw ('upstream COPYING-LGPL is missing: ' + $copying) }
$licenseTarget = Join-Path $libDir 'COPYING-LGPL'
Copy-Item $copying $licenseTarget -Force

# The shipped license text must be the one pinned in LICENSES/: guard against
# an upstream license change that the product would still label LGPLv3.
$pinnedLicense = Join-Path $repoRoot 'LICENSES/LGPL-3.0.txt'
if (-not (Test-Path $pinnedLicense)) { throw ('missing pinned license copy: ' + $pinnedLicense) }
$readNormalized = { param([string]$Path) ([IO.File]::ReadAllText($Path)).Replace($crlf, $lf).TrimEnd() }
if ((& $readNormalized $licenseTarget) -ne (& $readNormalized $pinnedLicense)) {
    throw 'the upstream COPYING-LGPL differs from LICENSES/LGPL-3.0.txt; review the license before shipping'
}

$libSha = Get-Sha256 $libTarget
$libSize = (Get-Item $libTarget).Length
if ($libSize -lt 1MB) {
    throw ('libwdi.lib is suspiciously small (' + $libSize + ' bytes); the WinUSB payload was probably not embedded')
}

$patchInfo = @()
foreach ($patch in $patches) {
    $patchInfo += [ordered]@{ name = $patch.Name; sha256 = (Get-Sha256 $patch.FullName) }
}
$toolsetVersion = ''
try {
    $msbuildVersion = @(& $msbuild '-version' 2>$null)
    if ($msbuildVersion.Count -gt 0) { $toolsetVersion = ([string]$msbuildVersion[$msbuildVersion.Count - 1]).Trim() }
} catch { }

$refType = [string]$env:GITHUB_REF_TYPE
$refName = [string]$env:GITHUB_REF_NAME
$artifactVersion = 'libwdi-1.5.1-x64-r1'
if ($refType -eq 'tag' -and $refName.StartsWith('libwdi-')) { $artifactVersion = $refName }

$info = [ordered]@{
    schema               = 1
    artifact             = 'libwdi-static'
    version              = $artifactVersion
    upstream             = 'pbatard/libwdi'
    upstream_tag         = 'v1.5.1'
    upstream_ref         = $LibwdiSha
    platform             = 'x64'
    payload              = 'WinUSB only (OPT_M32/OPT_ARM/LIBUSB0_DIR/LIBUSBK_DIR disabled)'
    runtime              = 'static CRT (/MT, DEFAULTLIB LIBCMT)'
    patches              = $patchInfo
    wdk_link_id          = '253170'
    wdk_msi_sha256       = $WdkMsiSha256.ToLowerInvariant()
    toolset_version      = $toolsetVersion
    runner_image         = ([string]$env:ImageOS)
    runner_image_version = ([string]$env:ImageVersion)
    built_at_utc        = ([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))
    lib_sha256           = $libSha
    lib_size             = $libSize
    license              = 'LGPL-3.0; COPYING-LGPL is part of this directory'
}
$buildInfoPath = Join-Path $libDir 'BUILD-INFO.json'
($info | ConvertTo-Json -Depth 4) | Set-Content -Path $buildInfoPath -Encoding utf8

# SHA256SUMS lists every file in lib/ except itself, as "<sha256>  <name>".
$sumsPath = Join-Path $libDir 'SHA256SUMS'
$entries = foreach ($file in (Get-ChildItem -Path $libDir -File | Where-Object { $_.Name -ne 'SHA256SUMS' } | Sort-Object Name)) {
    (Get-Sha256 $file.FullName) + '  ' + $file.Name
}
$entries | Set-Content -Path $sumsPath -Encoding ascii

foreach ($name in @('libwdi.lib', 'COPYING-LGPL', 'BUILD-INFO.json', 'SHA256SUMS')) {
    $path = Join-Path $libDir $name
    if (-not (Test-Path $path)) { throw ('artifact contract violated, missing ' + $path) }
}

# ---------------------------------------------------------------------------
# 6) Export for the workflow (C2 contract: LIBWDI_LIB_DIR)
# ---------------------------------------------------------------------------
if ($env:GITHUB_ENV) {
    ('LIBWDI_LIB_DIR=' + $libDir) | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
}
if ($env:GITHUB_OUTPUT) {
    ('lib_sha256=' + $libSha) | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    ('lib_dir=' + $libDir) | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}

Write-Host ''
Write-Host ('libwdi.lib : ' + $libTarget)
Write-Host ('sha256     : ' + $libSha)
Write-Host ('size       : ' + $libSize + ' bytes')
Write-Host ('artifact   : ' + $libDir)
