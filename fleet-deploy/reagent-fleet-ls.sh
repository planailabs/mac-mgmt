#!/bin/sh
# Just the instances in the reagent-fleet project on strontium.
set -eu
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  'incus list --project reagent-fleet --format csv -c ns4'
