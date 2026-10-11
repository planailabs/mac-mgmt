#!/bin/sh
# Liveness of the three fleet services plus the fleet's database state.
set -eu
. "$(dirname "$0")/lib.sh"

for entry in "xzar $XZAR_PORT /" "server $SERVER_PORT /api/server-info" "runner 9400 /status"; do
  set -- $entry
  code=$(curl -sS -m 30 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$2$3" 2>/dev/null || true)
  case "$code" in 2*|3*) ;; *) code="DOWN (${code:-no answer})" ;; esac
  echo "$1: $code"
done

echo "--- clusters ---"
psql "$MACMGMT_DSN" -Atc "select name from clusters order by created_at" 2>/dev/null || echo "(database unreachable)"
echo "--- heartbeats ---"
psql "$MACMGMT_DSN" -Atc "select instance_id, hostname, version, reported_at from daemon_heartbeats order by reported_at desc limit 10" 2>/dev/null || true
