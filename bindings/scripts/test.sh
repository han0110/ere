#!/usr/bin/env bash
# Run one language binding's test driver. Exits:
#   0   pass
#   99  skipped (toolchain missing)
#   *   real failure
#
# Requires bash 4+ (parent test_all.sh uses associative arrays).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINDINGS_DIR="$WORKSPACE_DIR/bindings"

missing() {
    echo "[$1] SKIPPED -- missing toolchain ($2)" >&2
    exit 99
}

case "${1:-}" in
    c)
        command -v cc    >/dev/null 2>&1 || missing c "cc"
        command -v cargo >/dev/null 2>&1 || missing c "cargo"
        (cd "$WORKSPACE_DIR" && cargo test --release -p ere-binding-c)
        ;;
    golang|go)
        command -v go    >/dev/null 2>&1 || missing golang "go"
        command -v cargo >/dev/null 2>&1 || missing golang "cargo (needed to build the C staticlib)"
        "$SCRIPT_DIR/compile.sh" golang
        (cd "$BINDINGS_DIR/golang" && go test ./...)
        ;;
    *) echo "usage: test.sh <c|golang>" >&2; exit 2 ;;
esac
