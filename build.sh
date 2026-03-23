#!/usr/bin/env bash
set -euxo pipefail

cargo build --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-apple-darwin

TMP=$(mktemp -d)
cp target/x86_64-unknown-linux-musl/release/mac-mgmt "$TMP/mac-mgmt-x86_64-linux"
cp target/aarch64-apple-darwin/release/mac-mgmt "$TMP/mac-mgmt-aarch64-mac"
tar czf mac-mgmt.tar.gz -C "$TMP" mac-mgmt-x86_64-linux mac-mgmt-aarch64-mac
rm -rf "$TMP"

echo "Created mac-mgmt.tar.gz"
