#!/usr/bin/env bash
#
# Build a macOS universal binary (arm64 + x86_64) for kddilck.
#
# Must run on macOS with Xcode Command Line Tools installed; cross-linking
# Mach-O binaries from Windows/Linux requires the Apple SDK, which is not
# redistributable, so this script intentionally refuses other hosts.
#
# Usage:
#   chmod +x build-macos.sh
#   ./build-macos.sh
#
# Output:
#   dist/kddilck   (universal Mach-O, ad-hoc signed when codesign is available)
#
set -euo pipefail

cd "$(dirname "$0")"

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
OUT_DIR=dist
OUT="$OUT_DIR/kddilck"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: a macOS universal binary must be built on macOS." >&2
    echo "       Windows/Linux hosts need the Apple SDK (headers + IOKit/" >&2
    echo "       CoreFoundation stubs), which cannot be redistributed." >&2
    echo "       Use a Mac, or run the GitHub Actions workflow instead." >&2
    exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
    echo "error: Xcode Command Line Tools not found; run: xcode-select --install" >&2
    exit 1
fi

rustup target add "${TARGETS[@]}"

for target in "${TARGETS[@]}"; do
    echo "==> cargo build --release --target $target"
    cargo build --release --target "$target"
done

mkdir -p "$OUT_DIR"

# Combine both slices. Each slice is already stripped by [profile.release].
lipo -create \
    "target/${TARGETS[0]}/release/kddilck" \
    "target/${TARGETS[1]}/release/kddilck" \
    -output "$OUT"

# Ad-hoc signature: required for arm64 executables and keeps Gatekeeper happy
# for locally built binaries. Remove if you sign with a Developer ID instead.
if command -v codesign >/dev/null 2>&1; then
    codesign --force --sign - "$OUT"
fi

echo
lipo -info "$OUT"
file "$OUT"
echo "Output: $OUT"
