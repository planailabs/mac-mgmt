#!/bin/sh
# Shared setup for the fleet-deploy scripts: load the site config from .env
# and derive the paths the scripts use. Source this, don't run it.
#
# Every script must be runnable from anywhere; it locates its own directory.
set -eu

_here="$(cd "$(dirname "$0")" && pwd)"
if [ -f "$_here/.env" ]; then
  # shellcheck disable=SC1091
  . "$_here/.env"
else
  echo "fleet-deploy: no .env in $_here — copy .env.example and fill it in" >&2
  exit 1
fi

fleet_dir="${FLEET_DIR:?FLEET_DIR must be set in .env}"
checkout="${CHECKOUT:?CHECKOUT must be set in .env}"

server_dir="$fleet_dir/server"
runner_dir="$fleet_dir/runner"
xzar_dir="$fleet_dir/xzar"
admin_token_file="$fleet_dir/admin.token"
state_file="$runner_dir/state.json"

# The server binary must run from dx's output dir: it serves ./public from cwd.
server_run_dir="$checkout/target/dx/mac-mgmt-server/release/web"

incus() {
  ssh -o BatchMode=yes "$INCUS_SSH" "$@"
}

# Run a command inside a fleet instance.
incus_exec() {
  _name="$1"; shift
  ssh -o BatchMode=yes "$INCUS_SSH" \
    "incus exec '$_name' --project '$INCUS_PROJECT' -- $*"
}
