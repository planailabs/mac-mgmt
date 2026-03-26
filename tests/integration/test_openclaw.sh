#!/bin/bash
# Test: OpenClaw service installs, setup runs, and starts

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

test_start "openclaw_starts_and_configured"

CONTAINER=$(create_container)
trap "destroy_container $CONTAINER" EXIT

push_binary "$CONTAINER"
push_config "$CONTAINER" "$SCRIPT_DIR/config.toml"

# Start daemon (handles nix install, setup, and spawning services)
echo "  Starting daemon..."
start_daemon "$CONTAINER"

# Wait for openclaw to be healthy
if ! wait_for "$CONTAINER" "openclaw health" "openclaw health --json" 300; then
    echo "  Daemon logs:"
    get_logs "$CONTAINER" | tail -50
    test_fail "openclaw_starts_and_configured" "openclaw health check did not pass"
    exit 1
fi

# Verify openclaw config exists
echo "  Checking openclaw config..."
if ! exec_in "$CONTAINER" "test -f /root/.openclaw/openclaw.json"; then
    test_fail "openclaw_starts_and_configured" "openclaw.json not created"
    exit 1
fi
echo "  openclaw.json exists"

# Verify openclaw is in nix profile
profile_output=$(exec_in "$CONTAINER" "nix profile list --json")
if assert_contains "$profile_output" "openclaw" "openclaw should be in nix profile"; then
    test_pass "openclaw_starts_and_configured"
else
    test_fail "openclaw_starts_and_configured" "openclaw not found in nix profile"
    exit 1
fi
