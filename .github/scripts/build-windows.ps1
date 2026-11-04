<#
.SYNOPSIS
    Build auroku locally on Windows (release).

.DESCRIPTION
    Uses the installed Rust host toolchain by default. Pass -Target to build a
    specific triple, for example x86_64-pc-windows-gnu.

    The C runtime is linked statically (crt-static) for both the CLI and GUI;
    Windows system DLLs such as kernel32/user32 (and opengl32 for the GUI) are
    still provided by the OS and cannot be static.

    A C compiler is required because rusb vendors libusb:
      - MSVC targets need Visual Studio Build Tools (Desktop development with C++)
      - GNU targets need MinGW-w64 (e.g. via MSYS2)

.EXAMPLE
    .\build-windows.ps1

.EXAMPLE
    .\build-windows.ps1 -Target x86_64-pc-windows-gnu

.OUTPUTS
    dist\auroku.exe
    dist\auroku-gui.exe
    dist\auroku-windows-<arch>.zip
#>
param(
    [string]$Target = "",
    # Unoptimised build with debug symbols (the CI "debug" branch sets
    # AUROKU_BUILD_DEBUG=1 instead).
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
# The script lives in .github/scripts; build from the repository root.
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

# ---------------------------------------------------------------------------
# Static CRT verification
# ---------------------------------------------------------------------------
# Everything except the operating system itself is linked statically (Rust
# std, libusb, libwdi). The PE import table must therefore never mention a
# dynamic C runtime or a sibling DLL.

function Find-Dumpbin {
    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $found = & $vswhere -latest -products * -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' 2>$null
        if ($found) { return @($found)[0] }
    }
    $command = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    return $null
}

function Assert-StaticRuntime {
    param(
        [string]$Exe,
        [string]$Dumpbin
    )

    $lines = @(& $Dumpbin /nologo /dependents $Exe)
    $header = $lines | Select-String -Pattern 'Image has the following dependencies' | Select-Object -First 1
    if (-not $header) { throw "unexpected dumpbin output for $Exe" }

    $imports = @()
    for ($i = $header.LineNumber; $i -lt $lines.Count; $i++) {
        $text = $lines[$i].Trim()
        if ([string]::IsNullOrWhiteSpace($text)) {
            if ($imports.Count -gt 0) { break }
            continue
        }
        if ($text -match '(?i)\.dll$') { $imports += $text.ToLowerInvariant() }
    }
    if ($imports.Count -eq 0) { throw "could not parse the import table of $Exe" }

    $banned = @(
        'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll',
        'concrt140.dll', 'ucrtbase.dll',
        'libgcc_s_seh-1.dll', 'libstdc++-6.dll', 'winpthread-1.dll'
    )
    $bad = $imports | Where-Object {
        ($banned -contains $_) -or ($_ -like 'vcruntime*') -or ($_ -like 'msvcp140*') -or ($_ -like 'api-ms-win-crt-*')
    }
    if ($bad) {
        throw ("{0} still imports a dynamic C runtime: {1}" -f $Exe, ($bad -join ', '))
    }

    Write-Host ("    {0}: {1} imports -> {2}" -f (Split-Path $Exe -Leaf), $imports.Count, ($imports -join ', '))
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo not found. Install Rust: https://rustup.rs'
}

if (-not $Target) {
    $Target = (rustc -vV | Select-String '^host:').ToString().Split(':')[1].Trim()
}

# Static-only build: link the C runtime into the executable.
if ($env:RUSTFLAGS) {
    $env:RUSTFLAGS = "$env:RUSTFLAGS -C target-feature=+crt-static"
} else {
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
}

$arch = switch -Wildcard ($Target) {
    'aarch64*' { 'arm64'; break }
    'i686*'    { 'x86'; break }
    default    { 'x86_64' }
}

# CI builds libwdi as a static library and points LIBWDI_LIB_DIR at it; with
# that variable present the Windows build also links the bundled WinUSB
# installer (no external wdi-simple.exe needed).
$features = 'gui'
if ($env:LIBWDI_LIB_DIR -and (Test-Path $env:LIBWDI_LIB_DIR)) {
    $features = 'gui,static-libwdi'
    Write-Host "==> static libwdi found in $env:LIBWDI_LIB_DIR"
}

$debugBuild = $Debug -or ($env:AUROKU_BUILD_DEBUG -eq '1')
if ($debugBuild) {
    $profileArgs = @()
    $profileDir = 'debug'
    $zipSuffix = '-debug'
    Write-Host '==> debug build (unoptimised, debug symbols)'
} else {
    $profileArgs = @('--release')
    $profileDir = 'release'
    $zipSuffix = ''
}

Write-Host "==> cargo build $($profileArgs -join ' ') --locked --target $Target --features $features --bins (static CRT)"
cargo build @profileArgs --locked --target $Target --features $features --bins

$built = Join-Path (Get-Location) "target\$Target\$profileDir\auroku.exe"
$builtGui = Join-Path (Get-Location) "target\$Target\$profileDir\auroku-gui.exe"
if (-not (Test-Path $built)) {
    throw "Build output not found: $built"
}
if (-not (Test-Path $builtGui)) {
    throw "Build output not found: $builtGui"
}

$distDir = Join-Path (Get-Location) 'dist'
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$distExe = Join-Path $distDir 'auroku.exe'
$distGuiExe = Join-Path $distDir 'auroku-gui.exe'
Copy-Item $built $distExe -Force
Copy-Item $builtGui $distGuiExe -Force

if ($Target -like '*-msvc') {
    $dumpbin = Find-Dumpbin
    if (-not $dumpbin) { throw 'dumpbin.exe not found; cannot verify the static C runtime' }
    Write-Host '==> Verifying static CRT (only OS DLLs may be imported)'
    Assert-StaticRuntime -Exe $distExe -Dumpbin $dumpbin
    Assert-StaticRuntime -Exe $distGuiExe -Dumpbin $dumpbin
}

# LGPLv3 compliance for the statically linked libwdi (see docs/windows-driver.md).
# The license text is part of the pinned libwdi-static artifact, so it is read
# from $env:LIBWDI_LIB_DIR/COPYING-LGPL. The old in-tree libwdi\COPYING-LGPL
# path only remains as a fallback for local development. When static-libwdi is
# active a missing license file is a hard error: packaging a statically linked
# LGPLv3 library without its license is not acceptable (the previous code
# silently skipped the file).
$archiveItems = @($distExe, $distGuiExe)
$license = $null
if ($env:LIBWDI_LIB_DIR) {
    $artifactLicense = Join-Path $env:LIBWDI_LIB_DIR 'COPYING-LGPL'
    if (Test-Path $artifactLicense) { $license = $artifactLicense }
}
if (-not $license) {
    $legacyLicense = Join-Path (Get-Location) 'libwdi\COPYING-LGPL'
    if (Test-Path $legacyLicense) {
        $license = $legacyLicense
        Write-Host "==> using the legacy in-tree libwdi license: $legacyLicense"
    }
}
if ($license) {
    $licenseCopy = Join-Path $distDir 'LICENSE-LGPL-3.0.txt'
    Copy-Item $license $licenseCopy -Force
    $archiveItems += $licenseCopy
    Write-Host "License: $licenseCopy"
} elseif ($features -like '*static-libwdi*') {
    throw 'static-libwdi is enabled but no COPYING-LGPL was found (looked in LIBWDI_LIB_DIR and libwdi\COPYING-LGPL); refusing to package an LGPLv3 static link without its license'
} else {
    Write-Host '==> libwdi is not linked; no LGPL license file to bundle'
}

# GPLv3 / third-party compliance files. Every archive must carry the project
# licence, the complete third-party notices and the LGPL texts for the static
# links (see THIRD-PARTY-NOTICES.md). Missing files are hard errors so a
# release can never be packaged without them.
$complianceRoot = Get-Location
$complianceFiles = @('LICENSE', 'THIRD-PARTY-NOTICES.md')
foreach ($name in $complianceFiles) {
    $source = Join-Path $complianceRoot $name
    if (-not (Test-Path $source)) { throw "required compliance file not found: $source" }
    $dest = Join-Path $distDir $name
    Copy-Item $source $dest -Force
    $archiveItems += $dest
    Write-Host "Compliance: $dest"
}

$licenseSourceDir = Join-Path $complianceRoot 'LICENSES'
foreach ($name in @('LGPL-3.0.txt', 'LGPL-2.1.txt')) {
    $source = Join-Path $licenseSourceDir $name
    if (-not (Test-Path $source)) { throw "required third-party license text not found: $source" }
}
$distLicenses = Join-Path $distDir 'LICENSES'
New-Item -ItemType Directory -Force -Path $distLicenses | Out-Null
Copy-Item (Join-Path $licenseSourceDir '*') $distLicenses -Recurse -Force
$archiveItems += $distLicenses
Write-Host "Compliance: $distLicenses"

$version = (Select-String -Path (Join-Path (Get-Location) 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$zip = Join-Path $distDir "auroku-windows-$arch-v$version$zipSuffix.zip"
Compress-Archive -Path $archiveItems -DestinationPath $zip -Force

# Assert the archive really contains the compliance files (Compress-Archive
# preserves the LICENSES directory and its entries use forward slashes).
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zipArchive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
    $entryNames = @($zipArchive.Entries | ForEach-Object { $_.FullName })
} finally {
    $zipArchive.Dispose()
}
foreach ($requiredEntry in @('LICENSE', 'THIRD-PARTY-NOTICES.md', 'LICENSES/LGPL-3.0.txt', 'LICENSES/LGPL-2.1.txt')) {
    if ($entryNames -notcontains $requiredEntry) {
        throw "archive $zip is missing the required compliance entry: $requiredEntry"
    }
}

Write-Host ""
Write-Host "Output:  $distExe"
Write-Host "Output:  $distGuiExe"
Write-Host "Archive: $zip"
