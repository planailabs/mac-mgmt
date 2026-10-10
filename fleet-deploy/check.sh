#!/bin/sh
# Quick liveness probe of the three services plus the fleet API.
set -u
for p in "xzar 17788 /" "server 7378 /api/self" "runner 9400 /status"; do
  set -- $p
  code=$(curl -sS -m 10 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$2$3" 2>/dev/null || echo ERR)
  echo "$1: $code"
done
code=$(curl -sS -m 10 -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $(cat /var/lib/reagent/fleet/admin.token)" http://127.0.0.1:7378/api/self 2>/dev/null || echo ERR)
echo "server(authed): $code"

echo "--- clusters ---"
psql -d macmgmt -Atc "select id, name from clusters order by created_at" 2>/dev/null
echo "--- heartbeats ---"
psql -d macmgmt -Atc "select instance_id, hostname, version, reported_at from daemon_heartbeats order by reported_at desc limit 10" 2>/dev/null
