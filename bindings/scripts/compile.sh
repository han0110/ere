#!/usr/bin/env bash
# Build one (or all) language bindings.
#
# Usage:
#   bindings/scripts/compile.sh         # build all bindings (c + golang)
#   bindings/scripts/compile.sh c       # build the C hub only
#   bindings/scripts/compile.sh golang  # build the C hub + stage it under bindings/golang/build/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINDINGS_DIR="$WORKSPACE_DIR/bindings"

OS_RAW="$(uname -s)"
case "$OS_RAW" in
    Linux*)               OS="Linux"   ;;
    Darwin*)              OS="Darwin"  ;;
    MINGW*|MSYS*|CYGWIN*) OS="Windows" ;;
    *) echo "unsupported OS: $OS_RAW" >&2; exit 2 ;;
esac
ARCH="$(uname -m)"

compile_c() {
    (cd "$WORKSPACE_DIR" && cargo build --release -p ere-binding-c)
    echo "[c] header: bindings/c/build/ere_verifier.h"
}

compile_golang() {
    "$SCRIPT_DIR/compile_to_native.sh" "$OS" "$ARCH" "ere_binding_c" "static" \
        "$BINDINGS_DIR/golang/build" "ere-binding-c"
    cp "$BINDINGS_DIR/c/build/ere_verifier.h" "$BINDINGS_DIR/golang/build/ere_verifier.h"
}

target="${1:-all}"
case "$target" in
    c)         compile_c ;;
    golang|go) compile_golang ;;
    # `compile_golang` already builds ere-binding-c (for a target triple),
    # which runs the cbindgen build script and emits the header. Calling
    # `compile_c` first would be redundant.
    all)       compile_golang ;;
    *) echo "unknown target: $target (want c|golang|all)" >&2; exit 2 ;;
esac
