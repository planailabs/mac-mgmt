#!/bin/sh
# Smoke-test the cloud-init endpoint: it must resolve a daemon version and
# mint a download URL that actually serves the daemon binary. Mirrors what the
# fleet runner does per cell (admin token + X-Cluster-Id header).
set -eu
TOKEN=$(cat /var/lib/reagent/fleet/admin.token)
CLUSTER=$(psql -d macmgmt -tAc "select id from clusters where name='mmr-hermes-ollama'")

printf '%s' '{"system":"x86_64-linux","server_url":"http://10.33.0.1:7378"}' > /tmp/ci-body.json
curl -fsS -m 30 -X POST http://127.0.0.1:7378/api/setting/cloud-init \
  -H "Authorization: Bearer $TOKEN" -H "X-Cluster-Id: $CLUSTER" \
  -H 'Content-Type: application/json' --data-binary @/tmp/ci-body.json > /tmp/cloud-init.json

python3 - <<'PY'
import json, re, subprocess, sys
d = json.load(open('/tmp/cloud-init.json'))
yaml = d['cloud_init']
print('cloud-init ok; sync_token length', len(d['sync_token']))
print(yaml)
url = re.search(r'https?://\S+/api/daemon-download/\S+', yaml)
if not url:
    sys.exit('no daemon-download url in cloud-init')
url = url.group(0).rstrip('"')
print('checking download url:', url)
code = subprocess.run(['curl', '-sS', '-m', '600', '-o', '/tmp/daemon-dl', '-w', '%{http_code}',
                       url.replace('10.33.0.1', '127.0.0.1')], capture_output=True, text=True).stdout
size = subprocess.run(['stat', '-c', '%s', '/tmp/daemon-dl'], capture_output=True, text=True).stdout.strip()
print(f'download: http {code}, {size} bytes')
PY
