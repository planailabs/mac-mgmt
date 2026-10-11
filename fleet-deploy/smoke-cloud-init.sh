#!/bin/sh
# End-to-end check of the bootstrap chain: mint a cloud-init for one cluster,
# then fetch the daemon download URL it advertises and confirm it serves the
# binary. Mirrors what the fleet runner does per cell.
set -eu
. "$(dirname "$0")/lib.sh"

cluster="${1:-$(psql -d macmgmt -tAc "select id from clusters order by created_at limit 1")}"
[ -n "$cluster" ] || { echo "no clusters yet" >&2; exit 1; }

body=$(mktemp)
trap 'rm -f "$body" /tmp/cloud-init.json' EXIT
printf '{"system":"%s","server_url":"%s"}' "$DAEMON_SYSTEM" "$SERVER_EXTERNAL_URL" > "$body"

curl -fsS -m 30 -X POST "http://127.0.0.1:$SERVER_PORT/api/setting/cloud-init" \
  -H "Authorization: Bearer $(cat "$admin_token_file")" \
  -H "X-Cluster-Id: $cluster" \
  -H 'Content-Type: application/json' --data-binary "@$body" > /tmp/cloud-init.json

python3 - "$SERVER_EXTERNAL_URL" <<'PY'
import json, re, subprocess, sys
server_url = sys.argv[1]
d = json.load(open('/tmp/cloud-init.json'))
yaml = d['cloud_init']
print('cloud-init ok; sync_token length', len(d['sync_token']))
print(yaml)
m = re.search(r'https?://\S+/api/daemon-download/\S+', yaml)
if not m:
    sys.exit('no daemon-download url in cloud-init')
url = m.group(0).rstrip('"')
print('checking download url:', url)
# Dial the loopback equivalent of the advertised URL from this host.
local = url.replace(server_url, f'http://127.0.0.1:{url.split(":")[-2].lstrip("0") or ""}')
local = re.sub(r'^http://(\[[^]]*\]|[^:/]+):(\d+)', r'http://127.0.0.1:\2', url)
code = subprocess.run(['curl', '-sS', '-m', '600', '-o', '/tmp/daemon-dl', '-w', '%{http_code}', local],
                      capture_output=True, text=True).stdout
size = subprocess.run(['stat', '-c', '%s', '/tmp/daemon-dl'], capture_output=True, text=True).stdout.strip()
print(f'download: http {code}, {size} bytes')
sys.exit(0 if code == '200' else 1)
PY
