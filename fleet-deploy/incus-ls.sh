#!/bin/sh
# Instances in the fleet's Incus project, with status and v4 address.
set -eu
. "$(dirname "$0")/lib.sh"

incus "incus list --project '$INCUS_PROJECT' --format csv -c ns4"
