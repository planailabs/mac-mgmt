#!/bin/bash
# Test: OpenClaw is configured for Ollama via extra_config merge

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

test_start "openclaw_configured_for_ollama"

CONTAINER=$(create_container)
trap "destroy_container $CONTAINER" EXIT

push_binary "$CONTAINER"
push_config "$CONTAINER" "$SCRIPT_DIR/config-with-extra.toml"

# Start daemon (handles nix install, setup, config merge, and spawning)
echo "  Starting daemon..."
start_daemon "$CONTAINER"

# Wait for both services to be healthy
if ! wait_for "$CONTAINER" "ollama health" "curl -sf http://127.0.0.1:11434/" 300; then
    echo "  Daemon logs:"
    get_logs "$CONTAINER" | tail -50
    test_fail "openclaw_configured_for_ollama" "ollama health endpoint did not respond"
    exit 1
fi

if ! wait_for "$CONTAINER" "openclaw health" "openclaw health --json" 300; then
    echo "  Daemon logs:"
    get_logs "$CONTAINER" | tail -50
    test_fail "openclaw_configured_for_ollama" "openclaw health check did not pass"
    exit 1
fi

# Verify the daemon performed the config merge by checking logs
echo "  Checking daemon logs for config merge..."
daemon_logs=$(get_logs "$CONTAINER")

if assert_contains "$daemon_logs" "extra_config merged" "daemon should log config merge"; then
    test_pass "openclaw_configured_for_ollama"
else
    echo "  Daemon logs (last 30 lines):"
    echo "$daemon_logs" | tail -30 | sed 's/^/    /'
    test_fail "openclaw_configured_for_ollama" "config merge not found in daemon logs"
    exit 1
fi
