#!/bin/sh
# Show strontium's firewall policy for traffic arriving on the incusbr0
# bridge, which is what drops the fleet's TCP to this container.
set -u
ssh -o BatchMode=yes root@strontium.y.mkg20001.io '
  echo "== nixos firewall units"
  systemctl is-active firewall.service 2>/dev/null
  echo "== nft: filter table chains"
  nft list table inet filter 2>/dev/null | head -60
'
