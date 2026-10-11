#!/bin/sh
# Show every dnsmasq host entry on incusbr0 that claims a name, and the
# instance behind it. Two projects sharing the bridge cannot reuse a name;
# this is what makes a launch fail with "Instance DNS name ... already used".
set -eu
ssh -o BatchMode=yes root@strontium.y.mkg20001.io '
  cd /var/lib/incus/networks/incusbr0/dnsmasq.hosts || exit 1
  for f in *; do
    printf "%-50s %s\n" "$f" "$(cat "$f")"
  done | grep -v "^\s*$" | sort
'
