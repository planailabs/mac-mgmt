#!/bin/sh
# Compare the HTTP path to the server over IPv6 from the Incus host and from
# inside a fleet instance (verbose, to see where the connection stalls).
set -u
V6="$(/var/lib/reagent/fleet/host-ipv6.sh)"
URL="http://[$V6]:7378/api/server-info"
echo "url: $URL"
echo "== from strontium host"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "curl -sS -m 15 -g -o /dev/null -w 'host: %{http_code}\n' '$URL' 2>&1" || true
echo "== from instance reagent-openclaw-ollama-n2-1 (verbose)"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "incus exec reagent-openclaw-ollama-n2-1 --project reagent-fleet -- \
     curl -sS -m 15 -g -v -o /dev/null '$URL' 2>&1 | tail -12" || true
