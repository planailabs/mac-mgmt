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

# A stale heartbeat alone says nothing: the daemon only re-reports every few
# minutes. What matters is whether an instance can reach the server at the URL
# cloud-init baked in (the runner's mgmt.public_url) — when it cannot, the
# cluster is deployable but dead, and only the runner's own view shows it.
echo "--- instance view ---"
code=$(curl -sS -m 30 -o /dev/null -w '%{http_code}' "http://127.0.0.1:9400/status" 2>/dev/null || true)
case "$code" in
  2*)
    curl -sS -m 30 http://127.0.0.1:9400/status | python3 -c '
import json, sys

d = json.load(sys.stdin)
cells = d["cells"]
for c in cells:
    if c.get("healthy") is False:
        print("  %s: unhealthy (%s)" % (c["key"], (c.get("detail") or "")[:60]))
print("  %d of %d cells without a fresh heartbeat"
      % (sum(1 for c in cells if c.get("healthy") is False), len(cells)))
' ;;
  *) echo "  runner DOWN (${code:-no answer})" ;;
esac
