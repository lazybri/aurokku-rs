#!/usr/bin/env bash
#
# Build auroku locally on Linux.
#
# The CLI is a fully static musl binary. The GUI is built against the host
# glibc and uses system graphics libraries at runtime (Wayland/X11/OpenGL),
# which cannot be fully static.
#
# Usage:
#   ./build-linux.sh
#   ./build-linux.sh --help
#
# Output:
#   dist/auroku-linux-x86_64       (fully static CLI)
#   dist/auroku-gui-linux-x86_64   (GUI, dynamic system graphics libraries)
#
set -euo pipefail

cd "$(dirname "$0")"

usage() {
    cat <<'EOF'
Usage: ./build-linux.sh

Builds a fully static x86_64 Linux CLI against musl and the host glibc GUI.

Requirements:
  - Rust toolchain with the x86_64-unknown-linux-musl target (rustup)
  - musl-gcc:
      Debian/Ubuntu: sudo apt-get install musl-tools linux-libc-dev
      Fedora:        sudo dnf install musl-gcc kernel-headers
      Arch:          sudo pacman -S musl linux-api-headers

Output:
  dist/auroku-linux-x86_64       static ELF, no runtime dependencies
  dist/auroku-gui-linux-x86_64   GUI; dynamic libc/libm/libgcc_s + system
                                 Wayland/X11/OpenGL at runtime
EOF
}

for arg in "$@"; do
    case "$arg" in
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

if ! command -v musl-gcc >/dev/null 2>&1; then
    echo "error: musl-gcc not found." >&2
    echo "  Debian/Ubuntu: sudo apt-get install musl-tools" >&2
    echo "  Fedora:        sudo dnf install musl-gcc" >&2
    echo "  Arch:          sudo pacman -S musl" >&2
    exit 1
fi

TARGET=x86_64-unknown-linux-musl

if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
        rustup target add "$TARGET"
    fi
fi

# musl-gcc compiles with -nostdinc and only exposes musl's own include tree,
# while libusb's netlink backend needs the kernel UAPI headers (linux/, asm/,
# asm-generic/). Expose the host UAPI headers through a private include
# directory instead of modifying the system musl installation (no sudo).
UAPI_DIR="target/musl-uapi"
rm -rf "$UAPI_DIR"
mkdir -p "$UAPI_DIR"

link_uapi() {
    local src=$1
    local name=$2
    if [[ -d "$src" ]]; then
        ln -s "$src" "$UAPI_DIR/$name"
        return 0
    fi
    return 1
}

if ! link_uapi /usr/include/linux linux; then
    echo "error: Linux UAPI headers not found (/usr/include/linux)." >&2
    echo "  Debian/Ubuntu: sudo apt-get install linux-libc-dev" >&2
    echo "  Fedora:        sudo dnf install kernel-headers" >&2
    echo "  Arch:          sudo pacman -S linux-api-headers" >&2
    exit 1
fi
link_uapi /usr/include/asm-generic asm-generic || true

if ! link_uapi /usr/include/asm asm; then
    if ! link_uapi /usr/include/x86_64-linux-gnu/asm asm; then
        echo "error: architecture UAPI headers not found (asm/)." >&2
        echo "  Install the kernel headers for your distribution." >&2
        exit 1
    fi
fi

# Keep libusb1-sys from linking the host glibc libudev into the static build.
export PKG_CONFIG=/bin/false
export CC_x86_64_unknown_linux_musl=musl-gcc
export AR_x86_64_unknown_linux_musl=ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
export C_INCLUDE_PATH="$PWD/$UAPI_DIR${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"

# --- CLI: fully static musl -------------------------------------------------
# Static-only build: statically link musl and the C runtime. A non-PIE
# relocation model avoids a PT_INTERP entry, so the executable is fully
# self-contained even when musl-gcc only provides Scrt1.o for -static-pie.
echo "==> cargo build --release --locked --target $TARGET --bin auroku (static)"
RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+crt-static -C relocation-model=static" \
    cargo build --release --locked --target "$TARGET" --bin auroku

OUT="target/$TARGET/release/auroku"

if ! file "$OUT" | grep -Eq 'statically linked|static-pie linked'; then
    echo "error: $OUT is not a static executable" >&2
    file "$OUT" >&2
    exit 1
fi

if command -v readelf >/dev/null 2>&1 && readelf -l "$OUT" | grep -q 'INTERP'; then
    echo "error: $OUT contains a dynamic interpreter (PT_INTERP)" >&2
    exit 1
fi

if command -v readelf >/dev/null 2>&1 && readelf -d "$OUT" | grep -q 'NEEDED'; then
    echo "error: $OUT contains dynamic dependencies (DT_NEEDED)" >&2
    exit 1
fi

# --- GUI: host glibc, minimal dynamic dependencies --------------------------
# A static musl GUI cannot work: winit/glutin load Wayland/X11/OpenGL with
# dlopen, and static musl does not support dynamic loading. Build the GUI
# against the host glibc and avoid libudev via PKG_CONFIG=/bin/false.
echo "==> cargo build --release --locked --features gui --bin auroku-gui (host glibc)"
env -u RUSTFLAGS -u C_INCLUDE_PATH \
    cargo build --release --locked --features gui --bin auroku-gui

GUI_OUT="target/release/auroku-gui"

if [[ ! -x "$GUI_OUT" ]]; then
    echo "error: GUI build output not found: $GUI_OUT" >&2
    exit 1
fi

mkdir -p dist
DIST="dist/auroku-linux-x86_64"
GUI_DIST="dist/auroku-gui-linux-x86_64"
cp "$OUT" "$DIST"
cp "$GUI_OUT" "$GUI_DIST"
chmod +x "$DIST" "$GUI_DIST"

echo
file "$OUT"
file "$GUI_OUT"
echo
echo "Static CLI: $DIST"
echo "GUI:        $GUI_DIST (dynamic system graphics libraries)"
echo
echo "Note: the GUI intentionally uses the host glibc; its Rust dependencies and"
echo "      libusb are static. Wayland/X11/OpenGL are loaded at runtime."
