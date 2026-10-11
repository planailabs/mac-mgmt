#!/bin/sh
# HTTP GET from inside a fleet instance, plus its routing table.
# Used to find the address instances can actually reach the server on.
# Usage: instance-curl.sh <instance> <url>
set -eu
. "$(dirname "$0")/lib.sh"

name="${1:?usage: instance-curl.sh <instance> <url>}"
url="${2:?usage: instance-curl.sh <instance> <url>}"

incus "incus exec '$name' --project '$INCUS_PROJECT' -- sh -c '
  curl -sS -m 20 -o /dev/null -w \"$url -> %{http_code}\n\" \"$url\" 2>&1
  echo \"== v6 route\"; ip -6 route | head -6
  echo \"== v4 route\"; ip -4 route | head -6
'"
