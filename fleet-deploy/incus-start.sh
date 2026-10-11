#!/bin/sh
# Start one instance in the reagent-fleet project and report what incus says.
# Used to tell a real launch failure (a bad profile/image/reference) apart from
# the runner's own bookkeeping.
set -u
name="${1:?usage: incus-start.sh <instance>}"
ssh -o BatchMode=yes root@strontium.y.mkg20001.io \
  "incus start '$name' --project reagent-fleet; echo rc=\$?; incus info --show-log '$name' --project reagent-fleet | tail -30"
