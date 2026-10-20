#!/usr/bin/env bash
#
# Build auroku locally on macOS.
#
# Note: macOS does not support fully static executables; libSystem and the
# system frameworks are always linked dynamically. All Rust crates and the
# vendored libusb are still linked statically, and no compile option can
# remove the remaining system dylib dependency.
#
# Must run on macOS with Xcode Command Line Tools installed; cross-linking
# Mach-O binaries from Windows/Linux requires the Apple SDK, which is not
# redistributable, so this script intentionally refuses other hosts.
#
# Usage:
#   chmod +x build-macos.sh
#   ./build-macos.sh             # universal binary (arm64 + x86_64)
#   ./build-macos.sh --native    # current machine architecture only (faster)
#   ./build-macos.sh --help
#
# Output:
#   dist/auroku           (CLI, Mach-O, ad-hoc signed when codesign is available)
#   dist/auroku-gui.app   (Qt 6 GUI bundle, ad-hoc signed when available)
#
set -euo pipefail

cd "$(dirname "$0")"

usage() {
    cat <<'EOF'
Usage: ./build-macos.sh [--native | --universal]

  --universal   Build arm64 + x86_64 and combine with lipo (default)
  --native      Build only the current machine architecture
  -h, --help    Show this help

Output:
  dist/auroku           CLI (universal or native)
  dist/auroku-gui.app   Qt 6 GUI bundle (native architecture)

The GUI requires Qt 6.5+ (for example `brew install qt`). The CLI is still a
fully self-contained Rust binary; the GUI is a normal Qt application bundle.
EOF
}

MODE=universal
for arg in "$@"; do
    case "$arg" in
        --universal) MODE=universal ;;
        --native) MODE=native ;;
        -h|--help) usage; exit 0 ;;
        *)
            echo "error: unknown argument: $arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: a macOS binary must be built on macOS." >&2
    echo "       Windows/Linux hosts need the Apple SDK (headers + IOKit/" >&2
    echo "       CoreFoundation stubs), which cannot be redistributed." >&2
    echo "       Use a Mac, or run the GitHub Actions workflow instead." >&2
    exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
    echo "error: Xcode Command Line Tools not found; run: xcode-select --install" >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found. Install Rust: https://rustup.rs" >&2
    exit 1
fi
OUT_DIR=dist
OUT="$OUT_DIR/auroku"
GUI_APP="$OUT_DIR/auroku-gui.app"
mkdir -p "$OUT_DIR"

if [[ "$MODE" == "native" ]]; then
    case "$(uname -m)" in
        arm64|aarch64) HOST_ARCH=aarch64 ;;
        x86_64) HOST_ARCH=x86_64 ;;
        *)
            echo "error: unsupported macOS architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac

    TARGET="${HOST_ARCH}-apple-darwin"
    if command -v rustup >/dev/null 2>&1; then
        if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
            rustup target add "$TARGET"
        fi
    fi

    echo "==> cargo build --release --locked --target $TARGET --bin auroku"
    cargo build --release --locked --target "$TARGET" --bin auroku
    cp "target/$TARGET/release/auroku" "$OUT"
else
    TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
    if command -v rustup >/dev/null 2>&1; then
        rustup target add "${TARGETS[@]}" >/dev/null
    fi

    for target in "${TARGETS[@]}"; do
        echo "==> cargo build --release --locked --target $target --bin auroku"
        cargo build --release --locked --target "$target" --bin auroku
    done

    # Combine both slices. Each slice is already stripped by [profile.release].
    lipo -create \
        "target/${TARGETS[0]}/release/auroku" \
        "target/${TARGETS[1]}/release/auroku" \
        -output "$OUT"
fi

chmod +x "$OUT"

# Ad-hoc signature: required for arm64 executables and keeps Gatekeeper happy
# for locally built binaries. Remove if you sign with a Developer ID instead.
if command -v codesign >/dev/null 2>&1; then
    codesign --force --sign - "$OUT"
fi

# ------------------------------------------------------------------------------
# Qt 6 GUI (built for the current machine architecture)
# ------------------------------------------------------------------------------
QT_PREFIX="${QT_PREFIX:-${QT_ROOT_DIR:-}}"
if [[ -z "$QT_PREFIX" ]] && command -v brew >/dev/null 2>&1; then
    QT_PREFIX="$(brew --prefix qt 2>/dev/null || true)"
fi

GUI_BUILT=0
if ! command -v cmake >/dev/null 2>&1; then
    echo "warning: cmake not found; skipping the Qt GUI." >&2
elif [[ -z "$QT_PREFIX" || ! -d "$QT_PREFIX" ]]; then
    echo "warning: Qt 6 not found; skipping the GUI." >&2
    echo "         Install it with: brew install qt" >&2
else
    GUI_BUILD="build/gui-macos"
    echo "==> cmake -S gui -B $GUI_BUILD -DCMAKE_PREFIX_PATH=$QT_PREFIX"
    cmake -S gui -B "$GUI_BUILD" -DCMAKE_BUILD_TYPE=Release \
        "-DCMAKE_PREFIX_PATH=$QT_PREFIX"
    echo "==> cmake --build $GUI_BUILD"
    cmake --build "$GUI_BUILD"

    GUI_EXE="$GUI_BUILD/auroku-gui.app/Contents/MacOS/auroku-gui"
    if command -v otool >/dev/null 2>&1 && otool -L "$GUI_EXE" | grep -q 'libauroku_ffi'; then
        echo "error: $GUI_EXE still depends on libauroku_ffi.dylib" >&2
        exit 1
    fi

    rm -rf "$GUI_APP"
    # Remove the bridge left in the bundle by older builds.
    rm -f "$GUI_BUILD/auroku-gui.app/Contents/MacOS/libauroku_ffi.dylib"
    cp -R "$GUI_BUILD/auroku-gui.app" "$GUI_APP"
    DEPLOYQT="$QT_PREFIX/bin/macdeployqt"
    if [[ -x "$DEPLOYQT" ]]; then
        "$DEPLOYQT" "$GUI_APP" -always-overwrite
    fi
    # Keep the offscreen/minimal platform plugins in the bundle so headless
    # smoke tests and CI runs work without a WindowServer session.
    for plugin in libqoffscreen.dylib libqminimal.dylib; do
        source="$QT_PREFIX/plugins/platforms/$plugin"
        if [[ -f "$source" && -d "$GUI_APP/Contents/PlugIns/platforms" ]]; then
            cp "$source" "$GUI_APP/Contents/PlugIns/platforms/"
        fi
    done
    if command -v codesign >/dev/null 2>&1; then
        codesign --force --deep --sign - "$GUI_APP"
    fi
    GUI_BUILT=1
fi

echo
if [[ "$MODE" == "universal" ]]; then
    lipo -info "$OUT"
fi
file "$OUT"
echo "Output: $OUT"
if [[ "$GUI_BUILT" -eq 1 ]]; then
    echo "Output: $GUI_APP (native architecture; needs Qt 6 runtime frameworks)"
else
    echo "GUI:    skipped"
fi

