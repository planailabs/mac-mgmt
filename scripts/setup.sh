#!/usr/bin/env bash

#curl -fsSL https://openclaw.ai/install.sh | bash -s -- --no-onboard

yes | sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon

echo "extra-experimental-features = nix-command flakes" | tee -a /etc/nix/nix.conf
