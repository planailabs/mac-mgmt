#!/usr/bin/env bash

set -euxo pipefail

if ! command -v skopeo >/dev/null 2>&1; then
  echo "error: skopeo is not available; run via 'nix develop -c bash docker-push.sh'" >&2
  echo "debug: PATH has $(printf '%s' "$PATH" | tr ':' '\n' | wc -l) entries" >&2
  exit 127
fi

copy_image() {
  local archive="$1"
  local destination="$2"
  local xtrace_was_on=0

  case "$-" in
    *x*) xtrace_was_on=1; set +x ;;
  esac

  skopeo copy \
    "docker-archive:${archive}" \
    "${destination}" \
    --dest-creds "${CI_REGISTRY_USER}:${CI_REGISTRY_PASSWORD}"

  if [ "$xtrace_was_on" -eq 1 ]; then
    set -x
  fi
}

# Ensure skopeo trust policy exists (CI runners may lack it).
mkdir -p /etc/containers 2>/dev/null || mkdir -p "$HOME/.config/containers"
POLICY_DIR="/etc/containers"
[ -w "$POLICY_DIR" ] || POLICY_DIR="$HOME/.config/containers"
[ -f "$POLICY_DIR/policy.json" ] || echo '{"default":[{"type":"insecureAcceptAnything"}]}' > "$POLICY_DIR/policy.json"

REGISTRY="${CI_REGISTRY:-git.plan.ai:5050}"
PROJECT="${CI_PROJECT_PATH:-plan-ai/mac-mgmt}"
TAG="${CI_COMMIT_SHORT_SHA:-latest}"

for img in server relay runner relay-ssh; do
  nix build ".#docker-${img}" -L
  archive="$(readlink -f result)"

  copy_image "$archive" "docker://${REGISTRY}/${PROJECT}/${img}:${TAG}"
  copy_image "$archive" "docker://${REGISTRY}/${PROJECT}/${img}:latest"
done

# NixOS-in-Docker test images (shared self-signed CA across all three)
for img in test-mac-mgmt-relay test-mac-mgmt-server test-mac-mgmt-daemon; do
  nix build ".#nixosConfigurations.${img}.config.system.build.dockerImage" -L
  archive="$(readlink -f result)"

  copy_image "$archive" "docker://${REGISTRY}/${PROJECT}/${img}:${TAG}"
  copy_image "$archive" "docker://${REGISTRY}/${PROJECT}/${img}:latest"
done
