#!/bin/sh
# Back-compat wrapper: the fleet's instances. See incus-ls.sh.
exec "$(dirname "$0")/incus-ls.sh" "$@"
