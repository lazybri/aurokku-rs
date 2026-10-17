#!/usr/bin/env bash
#
# Build auroku locally on macOS.
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
#   dist/auroku   (Mach-O, ad-hoc signed when codesign is available)
#
set -euo pipefail

cd "$(dirname "$0")"

usage() {
    cat <<'EOF'
Usage: ./build-macos.sh [--native | --universal]

  --universal   Build arm64 + x86_64 and combine with lipo (default)
  --native      Build only the current machine architecture
  -h, --help    Show this help

Output: dist/auroku
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

    echo "==> cargo build --release --locked --target $TARGET"
    cargo build --release --locked --target "$TARGET"
    cp "target/$TARGET/release/auroku" "$OUT"
else
    TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
    if command -v rustup >/dev/null 2>&1; then
        rustup target add "${TARGETS[@]}" >/dev/null
    fi

    for target in "${TARGETS[@]}"; do
        echo "==> cargo build --release --locked --target $target"
        cargo build --release --locked --target "$target"
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

echo
if [[ "$MODE" == "universal" ]]; then
    lipo -info "$OUT"
fi
file "$OUT"
echo "Output: $OUT"

