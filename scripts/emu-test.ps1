# RP2354 emulator mock tests (docs/plan/13 §1.6).
#
# Builds the test firmware for Cortex-M33, then runs the host harness that boots
# it in rp2350-emu with a scripted phone transport. On failures, inspect with
#   rp2350-emu-tui --elf <shown path>
$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$manifest = Join-Path $root 'firmware/rp2354-emu-fw/Cargo.toml'
$target = 'thumbv8m.main-none-eabihf'
$elf = Join-Path $root "firmware/rp2354-emu-fw/target/$target/release/auroku-rp2354-emu-fw"

Write-Host "==> building emulator firmware ($target)"
& cargo build --manifest-path $manifest --target $target --release
if ($LASTEXITCODE -ne 0) { throw "firmware build failed" }

$env:AUROKU_EMU_FW_ELF = $elf
$env:AUROKU_REQUIRE_EMU = '1'

Write-Host "==> running rp2350-emu mock tests"
& cargo test --manifest-path (Join-Path $root 'Cargo.toml') -p auroku-emu-tests -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "rp2350-emu mock tests failed" }
