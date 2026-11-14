#!/usr/bin/env bash

set -euo pipefail

if [ -z "${XZAR_TOKEN:-}" ]; then
  echo "XZAR_TOKEN is not set; skipping cache prebuild/upload job."
  echo "This is expected for pipelines that do not receive protected CI variables."
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/xzar-cache.sh
. "$SCRIPT_DIR/scripts/xzar-cache.sh"

# ── Configure xzar plan.ai cache ────────────────────────────────────
# Retries: the token check is an outbound call to xzar.plan.ai and a single
# transient 5xx there used to kill the job before the first build (job 8671).
xzar_add_server planai https://xzar.plan.ai "$XZAR_TOKEN"

upload() {
  while ! xzar --server planai upload --pin "$1" --desc "$(readlink -f "$2")" --leave-after-abandon 1m "$2"; do true; done
}

# ── Build devShell & push to xzar cache ─────────────────────────────
nix build .#devShells.x86_64-linux.default -o result-devshell
upload mac-mgmt/devshell result-devshell
rm -f result-devshell

# ── Build CI image & push to xzar cache ─────────────────────────────
nix build .#image -o result-image
upload mac-mgmt/image result-image
rm -f result-image

# ── Build all exposed packages & push to xzar cache ────────────────
for pkg in default server server-mgmt server-skill-center server-skill-importer \
           relay runner relay-ssh nix-driver-sync; do
  nix build ".#${pkg}" -o "result-${pkg}" -L
  upload "mac-mgmt/${pkg}" "result-${pkg}"
  rm -f "result-${pkg}"
done
