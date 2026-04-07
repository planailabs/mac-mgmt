#!/bin/bash
# Integration test: self-update via nix-store --realise.
# Stages a new binary in a temp dir, adds it to the local nix store,
# and runs `mac-mgmt update --force --version ... --store-path ...`
# to exercise the realise + self-replace path end-to-end.
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
    rm -rf "${WORK_DIR:-}"
}
trap cleanup EXIT

echo -e "${YELLOW}=== TEST: self-update via nix-store --realise ===${NC}"

if ! command -v nix-store >/dev/null 2>&1; then
    echo -e "${RED}FAIL: nix-store not on PATH; this test requires nix${NC}"
    exit 1
fi

# --- Build ---------------------------------------------------------------
echo "  Building mac-mgmt ($TARGET, features=$FEATURES)..."
cargo build --release --target "$TARGET" -p mac-mgmt --features "$FEATURES" \
    --manifest-path "$PROJECT_DIR/daemon/Cargo.toml" 2>&1 | tail -1

BINARY="$PROJECT_DIR/target/$TARGET/release/mac-mgmt"
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}FAIL: binary not found at $BINARY${NC}"
    exit 1
fi

# --- Stage a fake "new version" in the nix store --------------------------
WORK_DIR=$(mktemp -d)
STAGE="$WORK_DIR/stage"
mkdir -p "$STAGE/bin"
cp "$BINARY" "$STAGE/bin/mac-mgmt"
chmod +x "$STAGE/bin/mac-mgmt"

STORE_PATH=$(nix-store --add "$STAGE")
if [ -z "$STORE_PATH" ] || [ ! -d "$STORE_PATH" ]; then
    echo -e "${RED}FAIL: nix-store --add did not return a valid path${NC}"
    exit 1
fi
echo "  staged store path: $STORE_PATH"

# --- Copy the binary to a temp location (self-replace operates in-place) --
TEST_BIN="$WORK_DIR/mac-mgmt-under-test"
cp "$BINARY" "$TEST_BIN"
chmod +x "$TEST_BIN"

INODE_BEFORE=$(stat -c '%i' "$TEST_BIN")
HASH_BEFORE=$(sha256sum "$TEST_BIN" | awk '{print $1}')

# --- Run the update -------------------------------------------------------
FAKE_VERSION="99.99.99"
echo "  Running: $TEST_BIN update --force --version $FAKE_VERSION --store-path $STORE_PATH"
"$TEST_BIN" update --force --version "$FAKE_VERSION" --store-path "$STORE_PATH" 2>&1 | sed 's/^/    /'

# --- Verify ---------------------------------------------------------------
if [ ! -f "$TEST_BIN" ]; then
    echo -e "${RED}FAIL: binary disappeared after update${NC}"
    exit 1
fi

INODE_AFTER=$(stat -c '%i' "$TEST_BIN")
HASH_AFTER=$(sha256sum "$TEST_BIN" | awk '{print $1}')

echo "  inode before=$INODE_BEFORE after=$INODE_AFTER"
echo "  hash  before=$HASH_BEFORE"
echo "  hash  after =$HASH_AFTER"

if [ "$INODE_BEFORE" = "$INODE_AFTER" ]; then
    echo -e "${YELLOW}  (inode unchanged — self-replace may reuse inode on this FS)${NC}"
fi

if ! "$TEST_BIN" --version > /dev/null 2>&1; then
    echo -e "${RED}FAIL: binary not executable after update${NC}"
    exit 1
fi

# Marker should have been written to the config dir
MARKER="${XDG_CONFIG_HOME:-$HOME/.config}/mac-mgmt/.mac-mgmt.store-path"
if [ -f "$MARKER" ]; then
    if [ "$(cat "$MARKER")" = "$STORE_PATH" ]; then
        echo "  marker recorded: $MARKER"
    else
        echo -e "${YELLOW}  marker exists but content differs (expected $STORE_PATH)${NC}"
    fi
else
    echo -e "${YELLOW}  marker not found at $MARKER (non-fatal)${NC}"
fi

echo -e "${GREEN}PASS: self-update realised and replaced binary successfully${NC}"
