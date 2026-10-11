#!/bin/sh
# List the instances the runner has created in the reagent-fleet project.
set -eu
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  'incus list --project reagent-fleet --format csv -c n,s,4'
