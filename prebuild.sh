#!/usr/bin/env bash

set -euxo pipefail

# ── Configure xzar plan.ai cache ────────────────────────────────────
xzar config add-server planai https://xzar.plan.ai "$XZAR_TOKEN"

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
           relay runner relay-ssh web-agency web-agency-proxy nix-driver-sync; do
  nix build ".#${pkg}" -o "result-${pkg}" -L
  upload "mac-mgmt/${pkg}" "result-${pkg}"
  rm -f "result-${pkg}"
done
