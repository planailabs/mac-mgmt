#!/bin/sh
# Publish the locally built daemon binary to the local xzar cache as
# daemon/<version>/<system> and daemon/rolling/<system>, which is what
# POST /api/setting/cloud-init resolves to mint an instance's download URL.
# Mirrors what CI's xzar.sh does against the production cache.
set -eu
. "$(dirname "$0")/lib.sh"

bin="${BIN:-$checkout/target/release/mac-mgmt}"
[ -x "$bin" ] || { echo "missing daemon binary: $bin (cargo build --release -p mac-mgmt)" >&2; exit 1; }
[ -r "$xzar_dir/api.token" ] || { echo "missing xzar token: $xzar_dir/api.token" >&2; exit 1; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin"
cp "$bin" "$stage/bin/mac-mgmt"
chmod +x "$stage/bin/mac-mgmt"
store_path=$(nix-store --add "$stage")

xzar config add-server local "$XZAR_URL" "$(cat "$xzar_dir/api.token")" </dev/null
for pin in "daemon/$DAEMON_VERSION/$DAEMON_SYSTEM" "daemon/rolling/$DAEMON_SYSTEM"; do
  xzar --server local upload --pin "$pin" --desc "$store_path" \
    --leave-after-abandon 1m "$store_path" </dev/null
done
echo "published $store_path as daemon/$DAEMON_VERSION/$DAEMON_SYSTEM and daemon/rolling/$DAEMON_SYSTEM"
