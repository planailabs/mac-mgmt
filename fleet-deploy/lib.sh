#!/bin/sh
# Shared setup for the fleet-deploy scripts: load the site config from .env
# and derive the paths the scripts use. Source this, don't run it.
#
# Locates .env relative to the repo (this file's own directory), so callers
# work from any cwd — including a reagent service, whose $0 is not a path in
# the repo at all.
set -eu

# The fleet-deploy directory: prefer the caller's hint, else locate this file
# via $0. Reagent services run their script from outside the repo, so $0 is
# useless there — those must pass FLEET_DEPLOY_DIR.
if [ -n "${FLEET_DEPLOY_DIR:-}" ]; then
  _here="$FLEET_DEPLOY_DIR"
else
  _here="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
fi
if [ ! -f "$_here/lib.sh" ]; then
  echo "fleet-deploy: cannot find the repo's fleet-deploy dir (set FLEET_DEPLOY_DIR)" >&2
  exit 1
fi
if [ -f "$_here/.env" ]; then
  # shellcheck disable=SC1091
  . "$_here/.env"
else
  echo "fleet-deploy: no .env in $_here — copy .env.example and fill it in, or set FLEET_DEPLOY_DIR to the repo's fleet-deploy dir" >&2
  exit 1
fi

fleet_dir="${FLEET_DIR:?FLEET_DIR must be set in .env}"
checkout="${CHECKOUT:?CHECKOUT must be set in .env}"

# The nixpkgs xzar-server is a wrapper that exports XZAR_CONFIG, and XZAR_CONFIG
# overrides --config: it silently ignores the fleet's config.yaml and starts on
# upstream xzar.plan.ai settings instead (binds ::, wrong db role, wrong
# storage, wrong signing key), which makes the health check fail on a server
# that looks like it started fine. Fail loudly, here rather than only in
# setup.sh, so a bad path in .env cannot start anything unnoticed.
case "${XZAR_SERVER_BIN:?XZAR_SERVER_BIN must be set in .env}" in
  */nix/store/*-xzar-server/bin/xzar-server)
    echo "fleet-deploy: XZAR_SERVER_BIN is the nixpkgs wrapper, which ignores --config." >&2
    echo "              Point it at the unwrapped .../bin/xzar-server (see .env.example)." >&2
    exit 1 ;;
esac

server_dir="$fleet_dir/server"
runner_dir="$fleet_dir/runner"
xzar_dir="$fleet_dir/xzar"
# The per-user GC root for the unwrapped xzar-server binary. Nix puts an entry
# in /nix/var/nix/gcroots/auto pointing here, which is what keeps
# `nix-collect-garbage` from deleting the store path a running cache uses.
xzar_bin_root="${XDG_STATE_HOME:-$HOME/.local/state}/nix/profiles/xzar-server"
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
