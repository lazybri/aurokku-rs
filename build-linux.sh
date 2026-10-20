#!/usr/bin/env bash
#
# Build auroku locally on Linux.
#
# The CLI is a fully static musl binary. The Qt 6 GUI is a normal dynamically
# linked application; the static musl target only applies to the CLI.
#
# Usage:
#   ./build-linux.sh
#   ./build-linux.sh --help
#
# Output:
#   dist/auroku-linux-x86_64       (fully static CLI)
#   dist/auroku-gui-linux-x86_64   (Qt 6 GUI, Rust backend linked in)
#
set -euo pipefail

cd "$(dirname "$0")"

usage() {
    cat <<'EOF'
Usage: ./build-linux.sh

Builds a fully static x86_64 Linux CLI against musl and, when Qt 6 is
available, the C++ Qt Widgets GUI against the host glibc.

Requirements:
  - Rust toolchain with the x86_64-unknown-linux-musl target (rustup)
  - musl-gcc:
      Debian/Ubuntu: sudo apt-get install musl-tools linux-libc-dev
      Fedora:        sudo dnf install musl-gcc kernel-headers
      Arch:          sudo pacman -S musl linux-api-headers
  - GUI only: Qt 6.5+, CMake and Ninja
      Debian/Ubuntu: sudo apt-get install qt6-base-dev cmake ninja-build
      Fedora:        sudo dnf install qt6-qtbase-devel cmake ninja-build
      Arch:          sudo pacman -S qt6-base cmake ninja

Output:
  dist/auroku-linux-x86_64       static ELF, no runtime dependencies
  dist/auroku-gui-linux-x86_64   Qt 6 GUI; Rust backend linked in
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

# --- GUI: Qt 6 Widgets front-end --------------------------------------------
# The C++ GUI statically links the Rust backend (libauroku_ffi.a) and needs the
# Qt 6 development files. It cannot be built against musl, so the host
# toolchain is used. PKG_CONFIG=/bin/false avoids linking libudev into the Rust
# backend.
QT_PREFIX="${QT_PREFIX:-${QT_ROOT_DIR:-}}"
GUI_BUILD="build/gui-linux"
GUI_BIN="$GUI_BUILD/auroku-gui"
GUI_BUILT=0

if ! command -v cmake >/dev/null 2>&1 || ! command -v ninja >/dev/null 2>&1; then
    echo "warning: cmake/ninja not found; skipping the Qt GUI." >&2
    echo "         Debian/Ubuntu: sudo apt-get install cmake ninja-build" >&2
else
    CMAKE_ARGS=(-S gui -B "$GUI_BUILD" -G Ninja -DCMAKE_BUILD_TYPE=Release)
    if [[ -n "$QT_PREFIX" ]]; then
        CMAKE_ARGS+=("-DCMAKE_PREFIX_PATH=$QT_PREFIX")
    fi

    if env -u RUSTFLAGS -u C_INCLUDE_PATH cmake "${CMAKE_ARGS[@]}" >/dev/null; then
        echo "==> cmake --build $GUI_BUILD (Qt 6 GUI)"
        env -u RUSTFLAGS -u C_INCLUDE_PATH cmake --build "$GUI_BUILD"
        GUI_BUILT=1
    else
        echo "warning: Qt 6 development files not found; skipping the GUI." >&2
        echo "         Debian/Ubuntu: sudo apt-get install qt6-base-dev" >&2
        echo "         Fedora:        sudo dnf install qt6-qtbase-devel" >&2
        echo "         Arch:          sudo pacman -S qt6-base" >&2
    fi
fi

mkdir -p dist
# Remove the bridge left next to the GUI by older builds.
rm -f dist/libauroku_ffi.so
DIST="dist/auroku-linux-x86_64"
cp "$OUT" "$DIST"
chmod +x "$DIST"

echo
file "$OUT"
echo "Static CLI: $DIST"

if [[ "$GUI_BUILT" -eq 1 ]]; then
    GUI_DIST="dist/auroku-gui-linux-x86_64"

    if command -v readelf >/dev/null 2>&1; then
        if readelf -d "$GUI_BIN" | grep -q 'libauroku_ffi'; then
            echo "error: $GUI_BIN still depends on libauroku_ffi.so" >&2
            exit 1
        fi
        if readelf -d "$GUI_BIN" | grep -Fq "$PWD"; then
            echo "error: $GUI_BIN contains a build-machine path" >&2
            exit 1
        fi
    fi

    cp "$GUI_BIN" "$GUI_DIST"
    chmod +x "$GUI_DIST"
    file "$GUI_BIN"
    echo "GUI:        $GUI_DIST (Qt 6; Rust backend linked in)"
else
    echo "GUI:        skipped"
fi
