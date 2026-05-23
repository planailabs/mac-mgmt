#!/usr/bin/env bash
set -euo pipefail

RELEASE=0 ./build-memvault.sh
# Use the server binary that dx already built — shares the same compilation
# cache as the WASM client, preventing hydration mismatches.
cp target/dx/mac-mgmt/debug/web/server /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
sudo su -l daemon -s /bin/bash -c "env RUST_BACKTRACE=1 INPROCESS_SERVICE_MANAGER=1 /tmp/mac-mgmt daemon"
