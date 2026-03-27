#!/usr/bin/env bash
set -euxo pipefail

TARGETS=(
  x86_64-unknown-linux-musl
  aarch64-apple-darwin
)

cargo build --release --target "${TARGETS[0]}" -p mac-mgmt
cargo zigbuild --release --target "${TARGETS[1]}" -p mac-mgmt

TMP=$(mktemp -d)
for target in "${TARGETS[@]}"; do
  cp "../target/${target}/release/mac-mgmt" "$TMP/mac-mgmt-${target}"
done
tar czf mac-mgmt.tar.gz -C "$TMP" "${TARGETS[@]/#/mac-mgmt-}"
rm -rf "$TMP"

echo "Created mac-mgmt.tar.gz"
