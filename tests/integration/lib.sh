#!/bin/bash
# Shared test library for mac-mgmt integration tests

TESTS_PASSED=0
TESTS_FAILED=0
TESTS_TOTAL=0
CONTAINER_PREFIX="mac-mgmt-test"
BASE_IMAGE="mac-mgmt-base"
BINARY="target/x86_64-unknown-linux-musl/release/mac-mgmt"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Create an ephemeral test container from the base image
create_container() {
    local name="${CONTAINER_PREFIX}-$$-${RANDOM}"
    incus launch "$BASE_IMAGE" "$name" --ephemeral -c limits.memory=4GiB 2>&1 | sed 's/^/  [incus] /' >&2
    # Wait for container to be ready
    local retries=30
    while [ $retries -gt 0 ]; do
        if incus exec "$name" -- true 2>/dev/null; then
            break
        fi
        sleep 1
        retries=$((retries - 1))
    done
    if [ $retries -eq 0 ]; then
        echo -e "${RED}FAIL: container $name did not become ready${NC}" >&2
        return 1
    fi
    # Wait for networking
    retries=30
    while [ $retries -gt 0 ]; do
        if incus exec "$name" -- ping -c1 -W1 cache.nixos.org &>/dev/null; then
            break
        fi
        sleep 1
        retries=$((retries - 1))
    done
    echo "$name"
}

# Destroy a test container
destroy_container() {
    local name="$1"
    incus delete --force "$name" 2>/dev/null || true
}

# Clean up all test containers (for trap)
cleanup_all() {
    echo ""
    echo "Cleaning up test containers..."
    for c in $(incus list --format csv -c n 2>/dev/null | grep "^${CONTAINER_PREFIX}-"); do
        echo "  destroying $c"
        incus delete --force "$c" 2>/dev/null || true
    done
}

# Execute a command inside the container with nix environment
exec_in() {
    local container="$1"; shift
    local cmd="$*"
    incus exec "$container" -- bash -lc "source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null; export PATH=\"\$HOME/.nix-profile/bin:\$PATH\"; $cmd"
}

# Push the mac-mgmt binary into the container
push_binary() {
    local container="$1"
    incus file push "$PROJECT_DIR/$BINARY" "$container/root/mac-mgmt" --mode 0755
}

# Push a config file into the container
push_config() {
    local container="$1"
    local config_file="$2"
    incus exec "$container" -- mkdir -p /root/.config/mac-mgmt
    incus file push "$config_file" "$container/root/.config/mac-mgmt/config.toml"
}

# Run mac-mgmt setup inside the container (installs nix packages, configures services)
run_setup() {
    local container="$1"
    echo "  Running mac-mgmt setup..."
    exec_in "$container" "/root/mac-mgmt setup"
}

# Start the daemon in the background
start_daemon() {
    local container="$1"
    incus exec "$container" -- bash -c '
        source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null
        export PATH="$HOME/.nix-profile/bin:$PATH"
        nohup /root/mac-mgmt daemon > /tmp/mac-mgmt.log 2>&1 &
    '
}

# Get daemon logs
get_logs() {
    local container="$1"
    incus exec "$container" -- cat /tmp/mac-mgmt.log 2>/dev/null || true
}

# Wait for a health check to pass, with timeout
wait_for() {
    local container="$1"
    local description="$2"
    local check_cmd="$3"
    local timeout="${4:-180}"

    echo "  Waiting for $description (timeout: ${timeout}s)..."
    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        if exec_in "$container" "$check_cmd" &>/dev/null; then
            echo -e "  ${GREEN}$description ready after ${elapsed}s${NC}"
            return 0
        fi
        sleep 5
        elapsed=$((elapsed + 5))
    done

    echo -e "  ${RED}$description timed out after ${timeout}s${NC}"
    return 1
}

# Assertions
assert_eq() {
    local actual="$1"
    local expected="$2"
    local message="$3"
    if [ "$actual" = "$expected" ]; then
        return 0
    else
        echo -e "  ${RED}ASSERT FAILED: $message${NC}"
        echo "    expected: $expected"
        echo "    actual:   $actual"
        return 1
    fi
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    local message="$3"
    if echo "$haystack" | grep -q "$needle"; then
        return 0
    else
        echo -e "  ${RED}ASSERT FAILED: $message${NC}"
        echo "    expected to contain: $needle"
        echo "    actual: $haystack"
        return 1
    fi
}

# Test result tracking
test_start() {
    local name="$1"
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
    echo ""
    echo -e "${YELLOW}=== TEST: $name ===${NC}"
}

test_pass() {
    local name="$1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    echo -e "${GREEN}PASS: $name${NC}"
}

test_fail() {
    local name="$1"
    local reason="${2:-}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    echo -e "${RED}FAIL: $name${NC}"
    if [ -n "$reason" ]; then
        echo "  reason: $reason"
    fi
}

# Print final report
report_results() {
    echo ""
    echo "==============================="
    echo "  Results: $TESTS_PASSED/$TESTS_TOTAL passed"
    if [ $TESTS_FAILED -gt 0 ]; then
        echo -e "  ${RED}$TESTS_FAILED test(s) failed${NC}"
        return 1
    else
        echo -e "  ${GREEN}All tests passed${NC}"
        return 0
    fi
}
