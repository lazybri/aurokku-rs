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

Write-Host "==> cargo build --release --locked --target $Target --bin auroku (static CRT)"
cargo build --release --locked --target $Target --bin auroku

Write-Host "==> cargo build --release --locked --target $Target --features gui --bin auroku-gui (static CRT)"
cargo build --release --locked --target $Target --features gui --bin auroku-gui

$built = Join-Path $PSScriptRoot "target\$Target\release\auroku.exe"
$builtGui = Join-Path $PSScriptRoot "target\$Target\release\auroku-gui.exe"
if (-not (Test-Path $built)) {
    throw "Build output not found: $built"
}
if (-not (Test-Path $builtGui)) {
    throw "Build output not found: $builtGui"
}

$distDir = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$distExe = Join-Path $distDir 'auroku.exe'
$distGuiExe = Join-Path $distDir 'auroku-gui.exe'
Copy-Item $built $distExe -Force
Copy-Item $builtGui $distGuiExe -Force

$zip = Join-Path $distDir "auroku-windows-$arch.zip"
Compress-Archive -Path @($distExe, $distGuiExe) -DestinationPath $zip -Force

Write-Host ""
Write-Host "Output:  $distExe"
Write-Host "Output:  $distGuiExe"
Write-Host "Archive: $zip"
