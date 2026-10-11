#!/bin/sh
# Pins the local xzar cache holds, using the token the server is configured
# with. A pin named daemon/<version>/<system> is what cloud-init resolves to
# mint an instance's daemon download URL.
set -eu
. "$(dirname "$0")/lib.sh"

token=$(grep '^token' "$server_dir/config.toml" | cut -d'"' -f2)
curl -fsS -m 15 -H "Authorization: Bearer $token" "$XZAR_URL/pins" |
  python3 -c "
import json, sys
for p in json.load(sys.stdin):
    root = p['roots'][0]['drvFull'] if p['roots'] else '-'
    print(f\"{p['name']:<40} {root}{' (abandoned)' if p['abandoned'] else ''}\")"
