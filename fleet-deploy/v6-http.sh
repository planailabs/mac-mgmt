#!/bin/sh
# Compare the HTTP path to the server from the Incus host and from inside a
# fleet instance (verbose, to see where a connection stalls).
# Usage: v6-http.sh [instance]
set -eu
. "$(dirname "$0")/lib.sh"

name="${1:-}"
echo "url: $SERVER_EXTERNAL_URL"
echo "== from $INCUS_SSH"
incus "curl -sS -m 15 -g -o /dev/null -w 'host: %{http_code}\n' '$SERVER_EXTERNAL_URL/api/server-info' 2>&1" || true

if [ -n "$name" ]; then
  echo "== from instance $name (verbose)"
  incus "incus exec '$name' --project '$INCUS_PROJECT' -- \
    curl -sS -m 15 -g -v -o /dev/null '$SERVER_EXTERNAL_URL/api/server-info' 2>&1 | tail -12" || true
fi
