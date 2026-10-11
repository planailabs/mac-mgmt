#!/bin/sh
# Why an instance's cloud-init runcmd failed: the tail of the cloud-init
# output log and any daemon setup error.
# Usage: instance-bootstrap.sh <instance>
set -eu
. "$(dirname "$0")/lib.sh"

name="${1:?usage: instance-bootstrap.sh <instance>}"

incus "incus exec '$name' --project '$INCUS_PROJECT' -- sh -c '
  echo \"== cloud-init output (tail)\"
  tail -40 /var/log/cloud-init-output.log 2>&1
  echo \"== unit status\"
  systemctl status mac-mgmt --no-pager 2>&1 | head -20
  echo \"== setup, run by hand\"
  HOME=/root /usr/local/bin/mac-mgmt setup 2>&1 | tail -20
  echo \"== unit file\"
  ls -l /etc/systemd/system/mac-mgmt.service 2>&1
'"
