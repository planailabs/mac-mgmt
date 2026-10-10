#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
git add fleet-deploy scripts/publish-daemon-to-local-xzar.sh
git -c user.name=reagent -c user.email=reagent@localhost commit -q -m "fleet-deploy: local mac-mgmt server, xzar cache and runner for the reagent-fleet node

Adds the ops scripts and server config used to run a mac-mgmt management
server, a local xzar binary cache and a fleet runner against the
reagent-fleet Incus project on strontium. The daemon binary is published
to the local xzar as daemon/<version>/x86_64-linux, which is what
cloud-init resolves when minting instance bootstraps."
git log --oneline -1
