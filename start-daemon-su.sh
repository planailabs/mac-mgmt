#!/usr/bin/env bash
set -euo pipefail

cargo build -p mac-mgmt
cp target/debug/mac-mgmt /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
sudo su -l daemon -s /bin/sh -c "env RUST_BACKTRACE=1 RUST_LOG=debug INPROCESS_SERVICE_MANAGER=1 /tmp/mac-mgmt daemon"
