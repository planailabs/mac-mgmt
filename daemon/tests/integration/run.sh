#!/bin/bash
# Main integration test runner for mac-mgmt
# Usage: bash tests/integration/run.sh [test_name]
#   Run all tests:        bash tests/integration/run.sh
#   Run specific test:    bash tests/integration/run.sh ollama

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

# Cleanup on exit
trap cleanup_all EXIT

# Build the binary
echo "Building mac-mgmt (musl release)..."
cargo build --release --target x86_64-unknown-linux-musl -p mac-mgmt

if [ ! -f "$PROJECT_DIR/$BINARY" ]; then
    echo "ERROR: binary not found at $PROJECT_DIR/$BINARY"
    exit 1
fi

# Ensure base image exists
if ! incus image list --format csv -c l 2>/dev/null | grep -q "^${BASE_IMAGE}$"; then
    echo "Base image '$BASE_IMAGE' not found, building..."
    bash "$SCRIPT_DIR/setup-base-image.sh"
fi

# Determine which tests to run
FILTER="${1:-}"

PASSED=0
FAILED=0
TOTAL=0

run_test() {
    local test_file="$1"
    local test_name
    test_name=$(basename "$test_file" .sh | sed 's/^test_//')

    if [ -n "$FILTER" ] && [ "$test_name" != "$FILTER" ]; then
        return
    fi

    TOTAL=$((TOTAL + 1))
    if bash "$test_file"; then
        PASSED=$((PASSED + 1))
    else
        FAILED=$((FAILED + 1))
    fi
}

# Run tests
run_test "$SCRIPT_DIR/test_self_update.sh"
run_test "$SCRIPT_DIR/test_ollama.sh"
run_test "$SCRIPT_DIR/test_openclaw.sh"
run_test "$SCRIPT_DIR/test_config.sh"

# Report
echo ""
echo "==============================="
echo "  Results: $PASSED/$TOTAL passed"
if [ $FAILED -gt 0 ]; then
    echo -e "  ${RED}$FAILED test(s) failed${NC}"
    exit 1
else
    echo -e "  ${GREEN}All tests passed${NC}"
fi
