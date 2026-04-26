#!/usr/bin/env nix-shell
#! nix-shell -i bash -p skopeo

set -euxo pipefail

REGISTRY="${CI_REGISTRY:-git.plan.ai:5050}"
PROJECT="${CI_PROJECT_PATH:-plan-ai/mac-mgmt}"
TAG="${CI_COMMIT_SHORT_SHA:-latest}"

for img in server relay runner relay-ssh; do
  nix build ".#docker-${img}" -L

  skopeo copy \
    "docker-archive:$(readlink -f result)" \
    "docker://${REGISTRY}/${PROJECT}/${img}:${TAG}" \
    --dest-creds "${CI_REGISTRY_USER}:${CI_REGISTRY_PASSWORD}"

  skopeo copy \
    "docker-archive:$(readlink -f result)" \
    "docker://${REGISTRY}/${PROJECT}/${img}:latest" \
    --dest-creds "${CI_REGISTRY_USER}:${CI_REGISTRY_PASSWORD}"
done
