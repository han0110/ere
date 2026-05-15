#!/usr/bin/env bash
# Build a Rust binding crate for one target triple and copy the resulting
# artifact into a per-target subdirectory under OUT_DIR. On macOS, ARCH=universal
# additionally lipo-merges the x86_64 and aarch64 outputs.
#
# Args:
#   $1 OS         host OS (Linux | Darwin | Windows)
#   $2 ARCH       host arch (x86_64 | aarch64 | arm64 | universal)
#   $3 LIB_NAME   filesystem stem of the artifact (e.g. ere_binding_c)
#   $4 LIB_TYPE   static | dynamic
#   $5 OUT_DIR    absolute path; receives <OUT_DIR>/<target>/lib<LIB_NAME>.<ext>
#   $6 CARGO_PKG  cargo `-p` target (e.g. ere-binding-c)
set -euo pipefail

OS="${1:?missing OS}"
ARCH="${2:?missing ARCH}"
LIB_NAME="${3:?missing LIB_NAME}"
LIB_TYPE="${4:?missing LIB_TYPE}"
OUT_DIR="${5:?missing OUT_DIR}"
CARGO_PKG="${6:?missing CARGO_PKG}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Map (OS, ARCH) -> Rust target triple. `|` inside a `case` pattern is an
# OR-alternative, so nest the cases rather than concatenating "$OS|$ARCH".
target=""
case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)  target="x86_64-unknown-linux-gnu"  ;;
            aarch64) target="aarch64-unknown-linux-gnu" ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64)         target="x86_64-apple-darwin"  ;;
            arm64|aarch64)  target="aarch64-apple-darwin" ;;
            universal)      target="universal"            ;;
        esac
        ;;
    Windows)
        case "$ARCH" in
            x86_64) target="x86_64-pc-windows-gnu" ;;
        esac
        ;;
esac
if [[ -z "$target" ]]; then
    echo "unsupported OS/ARCH combination: OS='$OS' ARCH='$ARCH'" >&2
    exit 2
fi

case "$LIB_TYPE" in
    static)  ext="a";  prefix="lib" ;;
    dynamic)
        case "$OS" in
            Linux)   ext="so"    ; prefix="lib" ;;
            Darwin)  ext="dylib" ; prefix="lib" ;;
            Windows) ext="dll"   ; prefix=""    ;;
        esac
        ;;
    *) echo "LIB_TYPE must be static|dynamic, got '$LIB_TYPE'" >&2; exit 2 ;;
esac

build_one() {
    local triple="$1"
    rustup target add "$triple" >/dev/null 2>&1 || true
    echo "[compile] cargo build --release --target=$triple -p $CARGO_PKG"
    (cd "$WORKSPACE_DIR" && cargo build --release --target="$triple" -p "$CARGO_PKG")
    local artifact="$WORKSPACE_DIR/target/$triple/release/${prefix}${LIB_NAME}.${ext}"
    if [[ ! -f "$artifact" ]]; then
        echo "[compile] expected artifact missing: $artifact" >&2
        exit 1
    fi
    mkdir -p "$OUT_DIR/$triple"
    cp "$artifact" "$OUT_DIR/$triple/${prefix}${LIB_NAME}.${ext}"
}

if [[ "$target" == "universal" ]]; then
    build_one "x86_64-apple-darwin"
    build_one "aarch64-apple-darwin"
    out_subdir="universal-apple-darwin"
    mkdir -p "$OUT_DIR/$out_subdir"
    lipo -create -output "$OUT_DIR/$out_subdir/${prefix}${LIB_NAME}.${ext}" \
        "$OUT_DIR/x86_64-apple-darwin/${prefix}${LIB_NAME}.${ext}" \
        "$OUT_DIR/aarch64-apple-darwin/${prefix}${LIB_NAME}.${ext}"
else
    build_one "$target"
    out_subdir="$target"
fi

echo "[compile] OK: $OUT_DIR/$out_subdir/${prefix}${LIB_NAME}.${ext}"
