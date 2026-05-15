#!/usr/bin/env bash
# Iterate over every language binding and report the per-row outcome.
# Per-row exit codes are interpreted as PASS (0), SKIPPED (99), or FAILED (*).
# The script's own exit code is 0 only if no binding actually failed.
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINDINGS=(c golang)

declare -A RESULTS
overall=0

for binding in "${BINDINGS[@]}"; do
    echo "=== $binding ==="
    set +e
    "$SCRIPT_DIR/test.sh" "$binding"
    rc=$?
    set -e
    case "$rc" in
        0)  RESULTS[$binding]="PASS"        ;;
        99) RESULTS[$binding]="SKIPPED"     ;;
        *)  RESULTS[$binding]="FAILED($rc)" ; overall=1 ;;
    esac
done

echo
echo "Binding test summary"
echo "===================="
for binding in "${BINDINGS[@]}"; do
    printf "  %-7s %s\n" "$binding" "${RESULTS[$binding]}"
done

exit "$overall"
