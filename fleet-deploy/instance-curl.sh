#!/bin/sh
# Try an HTTP GET from inside a fleet instance, and show its v6 routing.
# Used to find the address fleet instances can actually reach the server on.
set -u
name="${1:?usage: instance-curl.sh <instance> <url>}"
url="${2:?usage: instance-curl.sh <instance> <url>}"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "incus exec '$name' --project reagent-fleet -- sh -c '
     curl -sS -m 20 -o /dev/null -w \"$url -> %{http_code}\n\" \"$url\" 2>&1
     echo \"== v6 route\"
     ip -6 route | head -6
     echo \"== v4 route\"
     ip -4 route | head -6
   '"
