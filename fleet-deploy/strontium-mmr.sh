#!/bin/sh
# Find what keeps recreating the mmr project's instances on strontium,
# so the `reagent-fleet` project can use the same names without colliding.
set -eu
ssh -o BatchMode=yes root@strontium.y.mkg20001.io '
  echo "== units mentioning mmr/incus"
  systemctl list-units --type=service --all --plain --no-legend 2>/dev/null | grep -iE "mmr|incus" || true
  echo "== systemd units whose name mentions mmr"
  ls /etc/systemd/system/ 2>/dev/null | grep -i mmr || true
  echo "== root crontab"
  crontab -l 2>/dev/null || true
  echo "== cron.d"
  ls /etc/cron.d 2>/dev/null || true
  echo "== processes mentioning mmr"
  ps -eo pid,etime,args | grep -i "mmr" | grep -v grep | cut -c1-160 || true
'
