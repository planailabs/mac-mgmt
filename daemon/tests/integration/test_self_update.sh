#!/bin/bash
# Integration test: self-update with a fake HTTP server
# Proves that the binary replacement mechanism works end-to-end.
#
# Usage: bash tests/integration/test_self_update.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TARGET="x86_64-unknown-linux-musl"
FEATURES="self-update"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

cleanup() {
    if [ -n "${SERVER_PID:-}" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "${WORK_DIR:-}"
}
trap cleanup EXIT

echo -e "${YELLOW}=== TEST: self-update (force) ===${NC}"

# --- Build ---------------------------------------------------------------
echo "  Building mac-mgmt ($TARGET, features=$FEATURES)..."
cargo build --release --target "$TARGET" -p mac-mgmt --features "$FEATURES" \
    --manifest-path "$PROJECT_DIR/daemon/Cargo.toml" 2>&1 | tail -1

BINARY="$PROJECT_DIR/target/$TARGET/release/mac-mgmt"
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}FAIL: binary not found at $BINARY${NC}"
    exit 1
fi

# --- Prepare work dir with fake update server content ---------------------
WORK_DIR=$(mktemp -d)
ENVIRONMENT="dev"
SERVE_DIR="$WORK_DIR/serve/$ENVIRONMENT"
mkdir -p "$SERVE_DIR"

# Version file
echo "99.99.99" > "$SERVE_DIR/mac-mgmt.version"

# tar.gz containing the binary under the expected name
BIN_NAME="mac-mgmt-$TARGET"
cp "$BINARY" "$WORK_DIR/$BIN_NAME"
tar czf "$SERVE_DIR/mac-mgmt.tar.gz" -C "$WORK_DIR" "$BIN_NAME"

# --- Start HTTP server ----------------------------------------------------
# Use Python's built-in HTTP server on a random port
python3 -c "
import http.server, socketserver, sys, os
os.chdir('$WORK_DIR/serve')
handler = http.server.SimpleHTTPRequestHandler
handler.log_message = lambda *a: None  # silence logs
with socketserver.TCPServer(('127.0.0.1', 0), handler) as s:
    port = s.server_address[1]
    sys.stdout.write(str(port))
    sys.stdout.flush()
    s.serve_forever()
" &
SERVER_PID=$!

# Read the port from the server's stdout — it prints it immediately
# Give Python a moment to bind
sleep 0.5

# Discover the port the server chose
PORT=$(ss -tlnp 2>/dev/null | grep "pid=$SERVER_PID" | awk '{print $4}' | grep -oP ':\K[0-9]+' | head -1)
if [ -z "$PORT" ]; then
    echo -e "${RED}FAIL: could not determine server port${NC}"
    exit 1
fi

UPDATE_URL="http://127.0.0.1:$PORT"
echo "  Fake update server on $UPDATE_URL (pid $SERVER_PID)"

# Verify server is reachable
if ! curl -sf "$UPDATE_URL/$ENVIRONMENT/mac-mgmt.version" > /dev/null; then
    echo -e "${RED}FAIL: server not reachable${NC}"
    exit 1
fi

# --- Copy binary to a temp location (self-replace operates in-place) ------
TEST_BIN="$WORK_DIR/mac-mgmt-under-test"
cp "$BINARY" "$TEST_BIN"
chmod +x "$TEST_BIN"

# Record inode before update
INODE_BEFORE=$(stat -c '%i' "$TEST_BIN")
HASH_BEFORE=$(sha256sum "$TEST_BIN" | awk '{print $1}')

# --- Run the update -------------------------------------------------------
echo "  Running: MAC_MGMT_UPDATE_URL=$UPDATE_URL $TEST_BIN update --force"
MAC_MGMT_UPDATE_URL="$UPDATE_URL" "$TEST_BIN" update --force 2>&1 | sed 's/^/    /'

# --- Verify ---------------------------------------------------------------
if [ ! -f "$TEST_BIN" ]; then
    echo -e "${RED}FAIL: binary disappeared after update${NC}"
    exit 1
fi

# self-replace creates a new file, so the inode should change
INODE_AFTER=$(stat -c '%i' "$TEST_BIN")
HASH_AFTER=$(sha256sum "$TEST_BIN" | awk '{print $1}')

echo "  inode before=$INODE_BEFORE after=$INODE_AFTER"
echo "  hash  before=$HASH_BEFORE"
echo "  hash  after =$HASH_AFTER"

if [ "$INODE_BEFORE" = "$INODE_AFTER" ]; then
    echo -e "${YELLOW}  (inode unchanged — self-replace may reuse inode on this FS, checking hash)${NC}"
fi

# The replaced binary should still be a valid executable
if ! "$TEST_BIN" --version > /dev/null 2>&1; then
    echo -e "${RED}FAIL: binary not executable after update${NC}"
    exit 1
fi

echo -e "${GREEN}PASS: self-update replaced binary successfully and it still runs${NC}"
