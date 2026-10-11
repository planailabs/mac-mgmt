#!/bin/sh
# Quick liveness probe of the three services plus the fleet database state.
set -u
for p in "xzar 17788 /" "server 7378 /api/server-info" "runner 9400 /status"; do
  set -- $p
  code=$(curl -sS -m 30 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$2$3" 2>/dev/null)
  case "$code" in 2*|3*) ;; *) code="ERR/$code" ;; esac
  echo "$1: $code"
done

echo "--- clusters ---"
psql -d macmgmt -Atc "select name from clusters order by created_at" 2>/dev/null
echo "--- heartbeats ---"
psql -d macmgmt -Atc "select instance_id, hostname, version, reported_at from daemon_heartbeats order by reported_at desc limit 10" 2>/dev/null
