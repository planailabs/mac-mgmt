#!/usr/bin/env bash
set -euo pipefail

RELEASE=0 ./build-memvault.sh
cargo build -p mac-mgmt
cp target/debug/mac-mgmt /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
cd /tmp
sudo sudo --login -u daemon env RUST_BACKTRACE=1 INPROCESS_SERVICE_MANAGER=1 $(which flamegraph) -o /tmp/daemon.flame.svg -- /tmp/mac-mgmt daemon
