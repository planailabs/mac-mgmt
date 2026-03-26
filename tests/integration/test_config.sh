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

failed=0

# Check daemon logs for config merge
echo "  Checking daemon logs for config merge..."
daemon_logs=$(get_logs "$CONTAINER")

if ! assert_contains "$daemon_logs" "extra_config merged" "daemon should log config merge"; then
    echo "  Daemon logs (last 30 lines):"
    echo "$daemon_logs" | tail -30 | sed 's/^/    /'
    failed=1
fi

# Verify extra_config was merged into openclaw.json
echo "  Checking openclaw.json content..."
openclaw_config=$(incus exec "$CONTAINER" -- cat /root/.openclaw/openclaw.json 2>/dev/null || true)

if ! assert_contains "$openclaw_config" "ollama" "openclaw.json should reference ollama"; then
    failed=1
fi

if ! assert_contains "$openclaw_config" "llm_provider" "openclaw.json should contain llm_provider"; then
    failed=1
fi

if ! assert_contains "$openclaw_config" "11434" "openclaw.json should contain ollama port"; then
    failed=1
fi

if [ $failed -eq 0 ]; then
    test_pass "openclaw_configured_for_ollama"
else
    echo "  Actual openclaw.json content:"
    echo "$openclaw_config" | sed 's/^/    /'
    test_fail "openclaw_configured_for_ollama" "config merge validation failed"
    exit 1
fi
