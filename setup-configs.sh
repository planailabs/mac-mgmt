#!/usr/bin/env bash
# Copy all config.example.toml files to config.toml without overriding existing ones.
set -euo pipefail

copy_if_missing() {
    local example="$1"
    local target="${example%.example.toml}.toml"
    if [ -f "$target" ]; then
        echo "skip: $target already exists"
    else
        cp "$example" "$target"
        echo "created: $target"
    fi
}

cd "$(dirname "$0")"

copy_if_missing server/config.example.toml
copy_if_missing relay/config.example.toml
copy_if_missing relay-ssh/config.example.toml
copy_if_missing daemon/config.example.toml
