#!/bin/sh
# Show the pins the local xzar cache holds, using the server's own token.
set -eu
TOKEN=$(grep '^token' /var/lib/reagent/fleet/server/config.toml | cut -d'"' -f2)
curl -fsS -m 15 -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17788/pins |
  python3 -c "import json,sys; [print(p['name'], p['roots'][0]['drvFull'] if p['roots'] else '-', 'abandoned' if p['abandoned'] else '') for p in json.load(sys.stdin)]"
