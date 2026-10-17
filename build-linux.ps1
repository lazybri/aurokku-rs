<#
Cross-compile the Linux (x86_64, static musl) binary from Windows.

Prerequisites:
  - Rust toolchain via rustup
  - Zig (provides Linux headers, musl libc and the lld linker)

Usage:
  .\build-linux.ps1                        # auto-detect zig under %USERPROFILE%\tools
  .\build-linux.ps1 -ZigPath C:\zig\zig.exe

Output: target\x86_64-unknown-linux-musl\release\kddilck (static ELF, no runtime deps)
#>
param(
    [string]$ZigPath = ""
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not $ZigPath -and $env:ZIG) { $ZigPath = $env:ZIG }
if (-not $ZigPath) {
    $z = Get-ChildItem "$env:USERPROFILE\tools\zig-*\zig.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if ($z) { $ZigPath = $z.FullName }
}
if (-not $ZigPath -or -not (Test-Path $ZigPath)) {
    throw 'zig.exe not found: install Zig, pass -ZigPath, or set the ZIG env var.'
}
$zig = (Resolve-Path $ZigPath).Path
$zigTarget = 'x86_64-linux-musl'
$rustTarget = 'x86_64-unknown-linux-musl'

rustup target add $rustTarget

# --- Generate Zig wrappers (rustc linker + cc-rs CC/AR) ---------------------
$wrapDir = Join-Path $PSScriptRoot 'target\zigwrap'
New-Item -ItemType Directory -Force -Path $wrapDir | Out-Null

$ccSrc = @"
use std::env;
use std::process::{exit, Command};

const ZIG: &str = r"$zig";

fn main() {
    let mut args: Vec<String> = Vec::new();
    let mut it = env::args().skip(1);
    const CRT: [&str; 9] = [
        "rcrt1.o", "crt1.o", "Scrt1.o", "crti.o", "crtn.o",
        "crtbegin.o", "crtbeginS.o", "crtend.o", "crtendS.o",
    ];
    while let Some(a) = it.next() {
        // cc-rs passes the Rust triple, which Zig cannot parse.
        if a == "--target" { it.next(); continue; }
        if a.starts_with("--target=") || a == "-target" { continue; }
        // Zig ignores -nostartfiles and supplies its own CRT, so drop Rust's
        // CRT objects to avoid duplicate symbol errors.
        if a == "-nostartfiles" || a == "-static-pie" { continue; }
        if let Some(name) = std::path::Path::new(&a).file_name().and_then(|n| n.to_str()) {
            if CRT.contains(&name) { continue; }
        }
        args.push(a);
    }
    let status = Command::new(ZIG)
        .arg("cc")
        .arg("-target")
        .arg("$zigTarget")
        .args(&args)
        .status()
        .expect("failed to spawn zig cc");
    exit(status.code().unwrap_or(1));
}
"@

$arSrc = @"
use std::env;
use std::process::{exit, Command};

const ZIG: &str = r"$zig";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let status = Command::new(ZIG)
        .arg("ar")
        .args(&args)
        .status()
        .expect("failed to spawn zig ar");
    exit(status.code().unwrap_or(1));
}
"@

$ccRs = Join-Path $wrapDir 'zigcc.rs'
$arRs = Join-Path $wrapDir 'zigar.rs'
Set-Content -Path $ccRs -Value $ccSrc -Encoding utf8
Set-Content -Path $arRs -Value $arSrc -Encoding utf8

$ccExe = Join-Path $wrapDir 'zigcc.exe'
$arExe = Join-Path $wrapDir 'zigar.exe'
rustc -O $ccRs -o $ccExe
rustc -O $arRs -o $arExe

# --- Build ------------------------------------------------------------------
$env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = $ccExe
$env:CC_x86_64_unknown_linux_musl = $ccExe
$env:AR_x86_64_unknown_linux_musl = $arExe

cargo build --release --target $rustTarget

Write-Host ""
Write-Host "Output: target\$rustTarget\release\kddilck"
