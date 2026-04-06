#!/usr/bin/env bash

set -euo pipefail

DEST="${1:?Usage: upload.sh <remote-folder>}"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "Uploading version ${VERSION} to ${DEST}"

SSH_KEY=$(mktemp)
echo "$ID_UPDATE" > "$SSH_KEY"
chmod 600 "$SSH_KEY"

SSH_OPTS="-i $SSH_KEY -o StrictHostKeyChecking=no"
REMOTE="deploy@logos.plan.ai"

# Create version directory on remote
ssh $SSH_OPTS "$REMOTE" "mkdir -p ${DEST}/${VERSION}"

# Upload archive to versioned path and update latest version pointer
echo "$VERSION" > mac-mgmt.version
rsync -e "ssh $SSH_OPTS" \
  mac-mgmt.tar.gz "${REMOTE}:${DEST}/${VERSION}/mac-mgmt.tar.gz"
rsync -e "ssh $SSH_OPTS" \
  mac-mgmt.version "${REMOTE}:${DEST}/mac-mgmt.version"

rm -f "$SSH_KEY"
echo "Uploaded ${DEST}/${VERSION}/mac-mgmt.tar.gz"
