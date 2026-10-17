<#
.SYNOPSIS
    Build auroku locally on Windows (release).

.DESCRIPTION
    Uses the installed Rust host toolchain by default. Pass -Target to build a
    specific triple, for example x86_64-pc-windows-gnu.

    A C compiler is required because rusb vendors libusb:
      - MSVC targets need Visual Studio Build Tools (Desktop development with C++)
      - GNU targets need MinGW-w64 (e.g. via MSYS2)

.EXAMPLE
    .\build-windows.ps1

.EXAMPLE
    .\build-windows.ps1 -Target x86_64-pc-windows-gnu

.OUTPUTS
    dist\auroku.exe
    dist\auroku-windows-<arch>.zip
#>
param(
    [string]$Target = ""
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo not found. Install Rust: https://rustup.rs'
}

if (-not $Target) {
    $Target = (rustc -vV | Select-String '^host:').ToString().Split(':')[1].Trim()
}

$arch = switch -Wildcard ($Target) {
    'aarch64*' { 'arm64'; break }
    'i686*'    { 'x86'; break }
    default    { 'x86_64' }
}

Write-Host "==> cargo build --release --locked --target $Target"
cargo build --release --locked --target $Target

$built = Join-Path $PSScriptRoot "target\$Target\release\auroku.exe"
if (-not (Test-Path $built)) {
    throw "Build output not found: $built"
}

$distDir = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$distExe = Join-Path $distDir 'auroku.exe'
Copy-Item $built $distExe -Force

$zip = Join-Path $distDir "auroku-windows-$arch.zip"
Compress-Archive -Path $distExe -DestinationPath $zip -Force

Write-Host ""
Write-Host "Output:  $distExe"
Write-Host "Archive: $zip"
