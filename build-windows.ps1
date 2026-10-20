<#
.SYNOPSIS
    Build auRokku on Windows: static-CRT CLI and the Qt 6 C++ GUI.

.DESCRIPTION
    Builds the Rust CLI with the installed host toolchain (or -Target), then
    configures and builds the Qt 6 Widgets front-end in gui/ through CMake.
    The Qt GUI links the Rust backend through the `auroku-ffi` C ABI DLL.

    Qt 6 (6.5+), CMake 3.21+ and Ninja are required for the GUI. If they are
    not found the script builds the CLI only and prints a warning.

    A C compiler is required because rusb vendors libusb:
      - MSVC targets need Visual Studio Build Tools (Desktop development with C++)
      - GNU targets need MinGW-w64 (e.g. via MSYS2)

.EXAMPLE
    .\build-windows.ps1

.EXAMPLE
    .\build-windows.ps1 -QtPrefix "C:\Qt\6.8.3\mingw_64"

.EXAMPLE
    .\build-windows.ps1 -SkipGui

.OUTPUTS
    dist\auroku.exe
    dist\auroku-gui-windows-<arch>\auroku-gui.exe (+ Qt runtime)
    dist\auroku-windows-<arch>.zip
#>
param(
    [string]$Target = "",
    [string]$QtPrefix = "",
    [switch]$SkipGui
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

$built = Join-Path $PSScriptRoot "target\$Target\release\auroku.exe"
if (-not (Test-Path $built)) {
    throw "Build output not found: $built"
}

$distDir = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$distExe = Join-Path $distDir 'auroku.exe'
Copy-Item $built $distExe -Force

# --- Verify the CLI is self-contained ----------------------------------------
# The CLI must only import DLLs shipped with Windows (Rust std, libusb and the
# C runtime are linked statically). Anything else means a MinGW/Qt/FFI runtime
# leaked into the binary.
$objdump = (Get-Command objdump -ErrorAction SilentlyContinue).Source
if (-not $objdump -and (Test-Path 'C:\msys64\mingw64\bin\objdump.exe')) {
    $objdump = 'C:\msys64\mingw64\bin\objdump.exe'
}
if ($objdump) {
    $imports = & $objdump -p $distExe |
        Select-String 'DLL Name:\s*(.+)$' |
        ForEach-Object { $_.Matches[0].Groups[1].Value.Trim() } |
        Sort-Object -Unique
    $nonSystem = $imports | Where-Object {
        $_ -notmatch '^(api-ms-|ext-ms-)' -and
        -not (Test-Path (Join-Path $env:SystemRoot "System32\$_"))
    }
    if ($nonSystem) {
        throw "auroku.exe has non-system DLL dependencies: $($nonSystem -join ', ')"
    }
    Write-Host "Verified: auroku.exe imports only Windows system DLLs ($($imports.Count) entries)."
} else {
    Write-Warning 'objdump not found; skipped the static dependency check for auroku.exe.'
}

# ------------------------------------------------------------------------------
# Qt GUI
# ------------------------------------------------------------------------------
function Find-QtPrefix {
    param([string[]]$Candidates)

    foreach ($candidate in $Candidates) {
        if ($candidate -and (Test-Path (Join-Path $candidate 'lib\cmake\Qt6\Qt6Config.cmake'))) {
            return (Resolve-Path $candidate).Path
        }
    }
    return ''
}

$qtCandidates = @($QtPrefix, $env:QT_ROOT_DIR, $env:Qt6_DIR, 'C:\msys64\mingw64')
if (Test-Path 'C:\Qt') {
    $qtCandidates += Get-ChildItem 'C:\Qt' -Directory |
        ForEach-Object { Get-ChildItem $_.FullName -Directory } |
        ForEach-Object { $_.FullName }
}
$resolvedQt = Find-QtPrefix -Candidates $qtCandidates

$guiDir = $null
if (-not $SkipGui) {
    $targetIsMsvc = $Target -match 'msvc'
    $qtIsMinGW = $resolvedQt -match 'mingw'

    $cmake = (Get-Command cmake -ErrorAction SilentlyContinue).Source
    if (-not $cmake -and (Test-Path 'C:\msys64\mingw64\bin\cmake.exe')) {
        $cmake = 'C:\msys64\mingw64\bin\cmake.exe'
    }
    $ninja = (Get-Command ninja -ErrorAction SilentlyContinue).Source
    if (-not $ninja -and (Test-Path 'C:\msys64\mingw64\bin\ninja.exe')) {
        $ninja = 'C:\msys64\mingw64\bin\ninja.exe'
    }

    if (-not $resolvedQt) {
        Write-Warning 'Qt 6 not found; skipping the GUI. Install Qt 6.5+ or pass -QtPrefix.'
    } elseif ($qtIsMinGW -eq $targetIsMsvc) {
        Write-Warning "Qt at $resolvedQt does not match target $Target (MinGW/MSVC mismatch); skipping the GUI."
    } elseif (-not $cmake -or -not $ninja) {
        Write-Warning 'CMake or Ninja not found; skipping the GUI.'
    } else {
        if ($qtIsMinGW -and (Test-Path 'C:\msys64\mingw64\bin')) {
            $env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
        }
        $env:PATH = "$resolvedQt\bin;$env:PATH"

        $buildDir = Join-Path $PSScriptRoot "build/gui-windows-$arch"
        Write-Host "==> cmake -S gui -B $buildDir -G Ninja -DCMAKE_PREFIX_PATH=$resolvedQt"
        & $cmake -S gui -B $buildDir -G Ninja -DCMAKE_BUILD_TYPE=Release `
            "-DCMAKE_PREFIX_PATH=$resolvedQt"
        if ($LASTEXITCODE -ne 0) {
            throw "CMake configure failed with exit code $LASTEXITCODE"
        }

        Write-Host "==> cmake --build $buildDir"
        & $cmake --build $buildDir
        if ($LASTEXITCODE -ne 0) {
            throw "Qt GUI build failed with exit code $LASTEXITCODE"
        }

        $guiDir = Join-Path $distDir "auroku-gui-windows-$arch"
        New-Item -ItemType Directory -Force -Path $guiDir | Out-Null
        Copy-Item (Join-Path $buildDir 'auroku-gui.exe') $guiDir -Force
        Copy-Item (Join-Path $buildDir 'auroku_ffi.dll') $guiDir -Force
        Get-ChildItem $buildDir -Filter 'Qt6*.dll' | Copy-Item -Destination $guiDir -Force
        foreach ($plugin in @('platforms', 'styles', 'imageformats', 'tls', 'generic', 'networkinformation')) {
            $source = Join-Path $buildDir $plugin
            if (Test-Path $source) {
                Copy-Item $source -Destination $guiDir -Recurse -Force
            }
        }

        # Headless testing / CI: ship the offscreen and minimal platform plugins
        # so `--smoke-test` works without an interactive desktop.
        $platformsDir = Join-Path $guiDir 'platforms'
        New-Item -ItemType Directory -Force -Path $platformsDir | Out-Null
        foreach ($platformPlugin in @('qoffscreen.dll', 'qminimal.dll')) {
            $pluginCandidates = @(
                (Join-Path $resolvedQt "plugins\platforms\$platformPlugin"),
                (Join-Path $resolvedQt "share\qt6\plugins\platforms\$platformPlugin")
            )
            foreach ($source in $pluginCandidates) {
                if (Test-Path $source) {
                    Copy-Item $source $platformsDir -Force
                    break
                }
            }
        }

        if ($qtIsMinGW) {
            foreach ($runtime in @('libgcc_s_seh-1.dll', 'libstdc++-6.dll', 'libwinpthread-1.dll')) {
                $source = Join-Path $resolvedQt "bin\$runtime"
                if (Test-Path $source) {
                    Copy-Item $source $guiDir -Force
                }
            }

            # windeployqt does not follow every MinGW dependency (ICU, harfbuzz,
            # freetype, …). Walk the import tables and copy non-system DLLs from
            # the Qt toolchain bin directory until the closure is complete.
            $objdump = Join-Path $resolvedQt 'bin\objdump.exe'
            if (-not (Test-Path $objdump)) {
                $objdump = (Get-Command objdump -ErrorAction SilentlyContinue).Source
            }
            if ($objdump) {
                $pending = [System.Collections.Generic.Queue[string]]::new()
                Get-ChildItem $guiDir -Recurse -File |
                    Where-Object { $_.Extension -in @('.exe', '.dll') } |
                    ForEach-Object { $pending.Enqueue($_.FullName) }
                $seen = @{}
                while ($pending.Count -gt 0) {
                    $file = $pending.Dequeue()
                    if ($seen.ContainsKey($file)) { continue }
                    $seen[$file] = $true

                    $imports = & $objdump -p $file 2>$null |
                        Select-String 'DLL Name:\s*(.+)$' |
                        ForEach-Object { $_.Matches[0].Groups[1].Value.Trim() }

                    foreach ($name in $imports) {
                        $target = Join-Path $guiDir $name
                        if (Test-Path $target) { continue }
                        if (Test-Path (Join-Path $env:SystemRoot "System32\$name")) { continue }
                        $source = Join-Path $resolvedQt "bin\$name"
                        if (Test-Path $source) {
                            Copy-Item $source $guiDir -Force
                            $pending.Enqueue($target)
                        } elseif ($name -notmatch '^(api-ms-|ext-ms-)') {
                            Write-Warning "Missing runtime DLL: $name"
                        }
                    }
                }
            }
        }
    }
}

# ------------------------------------------------------------------------------
# Package
# ------------------------------------------------------------------------------
$zip = Join-Path $distDir "auroku-windows-$arch.zip"
$payload = @($distExe)
if ($guiDir) {
    $payload += $guiDir
}
Compress-Archive -Path $payload -DestinationPath $zip -Force

Write-Host ""
Write-Host "Output:  $distExe"
if ($guiDir) {
    Write-Host "Output:  $guiDir\auroku-gui.exe"
} else {
    Write-Host "GUI:     skipped"
}