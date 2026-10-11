#!/bin/sh
# Where the server decides the Rocket listen address.
set -eu
cd /var/lib/reagent/worktrees/mac-mgmt/mac-mgmt-node-setup
echo "== config.rs api bits"
grep -n "address\|listen\|bind" server/src/config.rs | head -20
echo "== main.rs figment/rocket config"
grep -n "figment\|Config::\|address\|port(" server/src/main.rs | head -20
echo "== api/mod.rs build_rocket"
grep -n "figment\|Config::\|address\|port" server/src/api/mod.rs | head -20
