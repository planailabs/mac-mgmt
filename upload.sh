#!/usr/bin/env bash
set -euxo pipefail

DEST="${1:?Usage: upload.sh <remote-folder>}"

SSH_KEY=$(mktemp)
echo "$ID_UPDATE" > "$SSH_KEY"
chmod 600 "$SSH_KEY"

grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/' > mac-mgmt.version

rsync -e "ssh -i $SSH_KEY -o StrictHostKeyChecking=no" \
  mac-mgmt.tar.gz mac-mgmt.version "logos.plan.ai:${DEST}/"

rm -f "$SSH_KEY"
