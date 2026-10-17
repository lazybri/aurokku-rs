#!/usr/bin/env bash
#
# Build auroku locally on Linux.
#
# Usage:
#   ./build-linux.sh             # native release build (glibc)
#   ./build-linux.sh --musl      # static musl release build (x86_64)
#   ./build-linux.sh --help
#
# Output:
#   dist/auroku-linux-x86_64
#
set -euo pipefail

cd "$(dirname "$0")"

usage() {
    cat <<'EOF'
Usage: ./build-linux.sh [--musl]

  --musl        Build against musl (static ELF, x86_64)
  -h, --help    Show this help

Output: dist/auroku-linux-x86_64
EOF
}

MUSL=0
for arg in "$@"; do
    case "$arg" in
        --musl) MUSL=1 ;;
        -h|--help) usage; exit 0 ;;
        *)
            echo "error: unknown argument: $arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found. Install Rust: https://rustup.rs" >&2
    exit 1
fi

mkdir -p dist
DIST="dist/auroku-linux-x86_64"

if [[ "$MUSL" -eq 1 ]]; then
    TARGET=x86_64-unknown-linux-musl

    if ! command -v musl-gcc >/dev/null 2>&1; then
        echo "error: musl-gcc not found." >&2
        echo "  Debian/Ubuntu: sudo apt-get install musl-tools" >&2
        echo "  Fedora:        sudo dnf install musl-gcc" >&2
        exit 1
    fi

    if command -v rustup >/dev/null 2>&1; then
        if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
            rustup target add "$TARGET"
        fi
    fi

    # musl-gcc uses -nostdinc; libusb's linux_netlink.c needs the kernel UAPI
    # headers. Debian/Ubuntu musl packages do not always expose them.
    MUSL_INC=/usr/include/x86_64-linux-musl
    if [[ -d "$MUSL_INC" && ! -e "$MUSL_INC/linux/magic.h" ]]; then
        echo "warning: $MUSL_INC does not expose Linux UAPI headers." >&2
        echo "         If the build fails on <linux/magic.h>, run:" >&2
        echo "           sudo ln -sfn /usr/include/linux $MUSL_INC/linux" >&2
        echo "           sudo ln -sfn /usr/include/asm-generic $MUSL_INC/asm-generic" >&2
        echo "           sudo ln -sfn /usr/include/x86_64-linux-gnu/asm $MUSL_INC/asm" >&2
    fi

    # Keep libusb1-sys from linking the host glibc libudev into the static build.
    export PKG_CONFIG=/bin/false
    export CC_x86_64_unknown_linux_musl=musl-gcc
    export AR_x86_64_unknown_linux_musl=ar
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc

    echo "==> cargo build --release --locked --target $TARGET"
    cargo build --release --locked --target "$TARGET"

    OUT="target/$TARGET/release/auroku"

    if ! file "$OUT" | grep -Eq 'statically linked|static-pie linked'; then
        echo "error: $OUT is not a static executable" >&2
        exit 1
    fi
else
    echo "==> cargo build --release --locked"
    cargo build --release --locked
    OUT="target/release/auroku"
fi

cp "$OUT" "$DIST"
chmod +x "$DIST"

echo
file "$OUT"
echo "Output: $DIST"
