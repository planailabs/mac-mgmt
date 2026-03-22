#!/bin/sh

curl -fsSL https://openclaw.ai/install.sh | bash -s -- --no-onboard

yes | sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
