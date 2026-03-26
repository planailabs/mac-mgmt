#!/bin/bash
# Test: Ollama service installs, starts, and responds to health checks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

test_start "ollama_starts_and_healthy"

CONTAINER=$(create_container)
trap "destroy_container $CONTAINER" EXIT

push_binary "$CONTAINER"
push_config "$CONTAINER" "$SCRIPT_DIR/config.toml"

# Start daemon (handles nix install, setup, and spawning services)
echo "  Starting daemon..."
start_daemon "$CONTAINER"

# Wait for ollama health endpoint
if ! wait_for "$CONTAINER" "ollama health" "curl -sf http://127.0.0.1:11434/" 300; then
    echo "  Daemon logs:"
    get_logs "$CONTAINER" | tail -50
    test_fail "ollama_starts_and_healthy" "ollama health endpoint did not respond"
    exit 1
fi

# Verify ollama is in nix profile
echo "  Checking nix profile..."
profile_output=$(exec_in "$CONTAINER" "nix profile list --json")
if assert_contains "$profile_output" "ollama" "ollama should be in nix profile"; then
    test_pass "ollama_starts_and_healthy"
else
    test_fail "ollama_starts_and_healthy" "ollama not found in nix profile"
    exit 1
fi
