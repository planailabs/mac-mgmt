#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
git add -A fleet-deploy
git -c user.name=reagent -c user.email=reagent@localhost commit -q -F - <<'MSG'
fleet-deploy: daemon artifact notes, extra instance diagnostics

Records in the README why a locally built daemon cannot run on a fleet
instance (glibc interpreter pointing at /nix/store, which instances do not
have) and that CI publishes a musl build instead — the local musl recipe is
still failing on zstd_sys link errors, so pull the published artifact.

Adds instance-bootstrap.sh and instance-binary.sh, which is what pinned that
down, plus the nix/musl build attempts and a fleet-status overview.
MSG
git log --oneline -4
