#!/usr/bin/env bash

#curl -fsSL https://openclaw.ai/install.sh | bash -s -- --no-onboard

yes | sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon

echo "extra-experimental-features = nix-command flakes" | sudo tee -a /etc/nix/nix.conf

echo "substituters = https://cache.nixos.org/ https://xzar.plan.ai" | sudo tee -a /etc/nix/nix.conf
echo "trusted-public-keys = cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY= xzar.plan.ai:KUE66pjr6UX5HHCn9kedN1DJ2J5nSlBrKmE7tUjXewE=" | sudo tee -a /etc/nix/nix.conf

if [ "$(uname)" != "Darwin" ]; then
  sudo systemctl restart nix-daemon
fi
