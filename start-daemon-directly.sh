#!/usr/bin/env bash
set -euo pipefail

cargo build -p mac-mgmt
cp target/debug/mac-mgmt /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
env RUST_BACKTRACE=1 INPROCESS_SERVICE_MANAGER=1 /tmp/mac-mgmt daemon
