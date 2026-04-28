#!/usr/bin/env bash

#curl -fsSL https://openclaw.ai/install.sh | bash -s -- --no-onboard

yes | sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon

# Nix configuration (trusted-users + experimental features) is now handled by
# `mac-mgmt configure-os`. Binary cache substituters are provided dynamically
# by the server via GET /api/nix-caches.
# Run: mac-mgmt configure-os
