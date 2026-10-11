#!/bin/sh
# Point the runner at its own instance-name namespace. The `mmr` project on
# strontium owns mmr-* DNS names on the shared incusbr0 bridge, so a second
# fleet using those names cannot start.
set -eu
config=/var/lib/reagent/fleet/runner/config.toml
python3 - "$config" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read()
new = text.replace('name_prefix = "mmr-"', 'name_prefix = "reagent-"')
if new == text:
    print("name_prefix already set (or key missing):")
else:
    open(path, "w").write(new)
    print("updated name_prefix")
PY
grep '^name_prefix' "$config"
