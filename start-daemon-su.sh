#!/usr/bin/env bash
set -euo pipefail

RELEASE=0 ./build-memvault.sh
cargo build -p mac-mgmt
cp target/debug/mac-mgmt /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
sudo su -l daemon -s /bin/bash -c "env RUST_BACKTRACE=1 INPROCESS_SERVICE_MANAGER=1 /tmp/mac-mgmt daemon"
