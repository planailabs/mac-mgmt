#!/bin/sh
# What a freshly booted fleet instance did with its cloud-init: the daemon
# download, the installed binary, the unit, and the cloud-init runcmd log.
set -u
name="${1:?usage: instance-diag.sh <instance>}"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io "
  incus exec '$name' --project reagent-fleet -- sh -c '
    echo \"== mac-mgmt unit: \$(systemctl is-active mac-mgmt 2>&1)\"
    echo \"== binary: \$(ls -l /usr/local/bin/mac-mgmt 2>&1)\"
    echo \"== server reachable from the instance\"
    curl -sS -m 20 -o /dev/null -w \"server-info: %{http_code}\n\" http://10.33.0.1:7378/api/server-info 2>&1
    echo \"== daemon download\"
    curl -sS -m 60 -o /tmp/dl -w \"download: %{http_code}, %{size_download} bytes\n\" \
      \"\$(grep -o \"http://[^\\\"]*/api/daemon-download/[^\\\"]*\" /var/lib/cloud/instance/user-data.txt 2>/dev/null | head -1)\" 2>&1
    echo \"== runcmd output\"
    cat /var/log/cloud-init-output.log 2>&1 | tail -30
  '
"
