#!/bin/sh
# Commit the fleet-deploy changes. Kept as a script because this task's shell
# rejects multi-line commit messages passed straight through.
set -eu
cd "$(dirname "$0")/.."
git add fleet-deploy scripts/publish-daemon-to-local-xzar.sh server/src/api/mod.rs
git -c user.name=reagent -c user.email=reagent@localhost commit -q -F - <<'MSG'
server: bind the API on IPv6 too

Rocket was pinned to Ipv4Addr::UNSPECIFIED, so the API was unreachable over
IPv6. A mac-mgmt fleet whose instances live on a bridge that only routes
guest-to-guest over IPv6 (strontium's incusbr0) could not dial the server at
all. Ipv6Addr::UNSPECIFIED accepts both families.

Also adds fleet-deploy/ — the ops scripts and configs used to run the
management server, a local xzar binary cache and a fleet runner against the
reagent-fleet Incus project, plus the daemon publisher for the local cache.
MSG
git log --oneline -2
