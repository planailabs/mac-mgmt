#!/bin/sh
# Diagnose IPv6 reachability between this container, the Incus host and a
# fleet instance: who can ping whom, and does the neighbour entry resolve.
set -u
V6="$(/var/lib/reagent/fleet/host-ipv6.sh)"
echo "this container v6: $V6"
echo "== ping from this container"
ping -6 -c 2 -W 2 "$V6" 2>&1 | tail -3 || true
echo "== neighbours on eth0"
ip -6 neigh show dev eth0 2>&1 | head -5 || true
echo "== ping from strontium host"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io "ping -6 -c 2 -W 2 '$V6' 2>&1 | tail -3" || true
echo "== instance v6"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "incus exec reagent-openclaw-ollama-n2-1 --project reagent-fleet -- sh -c '
     ip -6 addr show eth0 | grep inet6
     ping -6 -c 2 -W 2 \"$V6\" 2>&1 | tail -3
   '" || true
