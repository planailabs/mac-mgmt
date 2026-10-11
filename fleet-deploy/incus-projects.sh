#!/bin/sh
# Every instance in every Incus project on the host. Two projects sharing a
# bridge cannot reuse an instance name, which is what makes a launch fail with
# "Instance DNS name ... already used on network".
set -eu
. "$(dirname "$0")/lib.sh"

incus "for p in \$(incus project list --format csv -c n | sed 's/ (current)//'); do
     echo \"== \$p\"
     incus list --project \"\$p\" --format csv -c ns 2>/dev/null || true
   done"
