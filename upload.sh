#!/usr/bin/env bash
set -euxo pipefail

DEST="${1:?Usage: upload.sh <remote-folder>}"

SSH_KEY=$(mktemp)
echo "$ID_UPDATE" > "$SSH_KEY"
chmod 600 "$SSH_KEY"

rsync -e "ssh -i $SSH_KEY -o StrictHostKeyChecking=no" \
  mac-mgmt.tar.gz "logos.plan.ai:${DEST}/mac-mgmt.tar.gz"

rm -f "$SSH_KEY"
