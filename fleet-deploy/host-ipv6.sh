#!/bin/sh
# This container's global IPv6 on the Incus bridge: the address fleet
# instances dial the management server on (guest-to-guest IPv4 does not work
# on incusbr0, global IPv6 does).
set -eu
ip -6 addr show eth0 scope global | grep -oP 'inet6 \K[0-9a-f:]+' | head -1
