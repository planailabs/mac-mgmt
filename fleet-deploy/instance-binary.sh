#!/bin/sh
# Why an instance's daemon binary won't run: its interpreter and library
# dependencies, checked inside the instance.
# Usage: instance-binary.sh <instance>
set -eu
. "$(dirname "$0")/lib.sh"

name="${1:?usage: instance-binary.sh <instance>}"

incus "incus exec '$name' --project '$INCUS_PROJECT' -- sh -c '
  echo \"== file type\"
  file /usr/local/bin/mac-mgmt 2>&1 | head -2
  echo \"== needed libraries\"
  ldd /usr/local/bin/mac-mgmt 2>&1 | head -20
  echo \"== interpreter present?\"
  head -c 200 /usr/local/bin/mac-mgmt | strings | head -3
  echo \"== try running it\"
  /usr/local/bin/mac-mgmt --version 2>&1 | head -5
'"
