#!/usr/bin/env bash
set -euo pipefail

cargo build -p mac-mgmt
cp target/debug/mac-mgmt /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt
sudo su daemon -s /bin/sh -c "/tmp/mac-mgmt daemon"
