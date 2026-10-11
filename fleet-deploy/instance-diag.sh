#!/bin/sh
# What a freshly booted instance did with its cloud-init: whether it reaches
# the server, whether the daemon downloaded and installed, and the unit state.
# Usage: instance-diag.sh <instance>
set -eu
. "$(dirname "$0")/lib.sh"

name="${1:?usage: instance-diag.sh <instance>}"

incus "incus exec '$name' --project '$INCUS_PROJECT' -- sh -c '
  echo \"== mac-mgmt unit: \$(systemctl is-active mac-mgmt 2>&1)\"
  echo \"== binary: \$(ls -l /usr/local/bin/mac-mgmt 2>&1)\"
  echo \"== server reachable from the instance\"
  curl -sS -m 20 -o /dev/null -w \"$SERVER_EXTERNAL_URL/api/server-info -> %{http_code}\n\" \
    \"$SERVER_EXTERNAL_URL/api/server-info\" 2>&1
  echo \"== same over the bridge gateway (for comparison)\"
  curl -sS -m 20 -o /dev/null -w \"$BRIDGE_GATEWAY_URL/api/server-info -> %{http_code}\n\" \
    \"$BRIDGE_GATEWAY_URL/api/server-info\" 2>&1
  echo \"== daemon download\"
  curl -sS -m 60 -o /tmp/dl -w \"download: %{http_code}, %{size_download} bytes\n\" \
    \"\$(grep -o \"http[^\\\"]*/api/daemon-download/[^\\\"]*\" /var/lib/cloud/instance/user-data.txt 2>/dev/null | head -1)\" 2>&1
  echo \"== cloud-init\"
  cloud-init status --long 2>&1 | head -12
  echo \"== journal\"
  journalctl -u mac-mgmt -n 20 --no-pager 2>&1 | tail -20
'"
