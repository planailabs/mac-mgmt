#!/bin/sh
# Which instances exist in which Incus project, on strontium.
set -eu
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "for p in \$(incus project list --format csv -c n | sed 's/ (current)//'); do
     echo \"== \$p\"
     incus list --project \"\$p\" --format csv -c ns 2>/dev/null || true
   done"
