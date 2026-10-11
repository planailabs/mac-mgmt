#!/bin/sh
# The inbound TCP ports the fleet needs from the incusbr0 subnet
# (10.33.0.0/24 and 2a01:4f8:c012:2caf:3010::/80), and what is listening.
set -u
echo "== listening TCP sockets"
ss -lnt 2>/dev/null
echo
echo "== what the fleet talks to"
cat <<'EOF'
17788   local xzar binary cache   -> server fetches pins; daemons pull the nix binary cache
7378    mac-mgmt server API       -> instances poll config/heartbeat/probes, fetch the daemon binary
7377    mac-mgmt server web UI    -> not needed by instances (operator only)
9396    daemon metrics (Prometheus) -> optional, scraped from outside
22      relay/ssh                 -> only if the runner needs it; not used by this fleet
EOF
