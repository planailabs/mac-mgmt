#!/usr/bin/env bash
# Publish the locally built daemon binary to the local xzar cache as
# daemon/<version>/<system> (and daemon/rolling/<system>), which is what
# POST /api/setting/cloud-init resolves to mint the instance download URL.
# Mirrors what CI's xzar.sh does against the production cache.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XZAR_URL="${XZAR_URL:-http://127.0.0.1:17788}"
XZAR_TOKEN_FILE="${XZAR_TOKEN_FILE:-/var/lib/reagent/fleet/xzar/api.token}"
BIN="${BIN:-$SCRIPT_DIR/target/release/mac-mgmt}"

[ -x "$BIN" ] || { echo "missing daemon binary: $BIN (cargo build --release -p mac-mgmt)" >&2; exit 1; }
[ -r "$XZAR_TOKEN_FILE" ] || { echo "missing xzar token: $XZAR_TOKEN_FILE" >&2; exit 1; }

VERSION="$(grep '^version' "$SCRIPT_DIR/daemon/Cargo.toml" | head -1 | cut -d'"' -f2)"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/bin"
cp "$BIN" "$STAGE/bin/mac-mgmt"
chmod +x "$STAGE/bin/mac-mgmt"
STORE_PATH="$(nix-store --add "$STAGE")"

xzar config add-server local "$XZAR_URL" "$(cat "$XZAR_TOKEN_FILE")"
for pin in "daemon/$VERSION/x86_64-linux" "daemon/rolling/x86_64-linux"; do
  xzar --server local upload --pin "$pin" --desc "$STORE_PATH" --leave-after-abandon 1m "$STORE_PATH"
done
echo "published $STORE_PATH as daemon/$VERSION/x86_64-linux and daemon/rolling/x86_64-linux"
