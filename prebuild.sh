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
