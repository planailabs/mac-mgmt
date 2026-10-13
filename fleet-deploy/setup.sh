#!/bin/sh
# One-time (idempotent) setup of a fleet deployment: directories, the Incus
# client certificate and its trust on the host, the postgres databases, the
# server/runner/xzar configs, then the token and version seed.
#
# Assumes the binaries are already built (see README.md in this directory).
set -eu
. "$(dirname "$0")/lib.sh"

# The wrapper check for XZAR_SERVER_BIN is in lib.sh, so it covers every user
# of that value, not only this script.
echo "== xzar binary"
[ -x "$XZAR_SERVER_BIN" ] || { echo "XZAR_SERVER_BIN not executable: $XZAR_SERVER_BIN" >&2; exit 1; }

echo "== directories"
mkdir -p "$server_dir" "$runner_dir" "$xzar_dir/storage" "$fleet_dir/state"
chmod 700 "$fleet_dir"

# The unwrapped store path has no referrer, so it is not a GC root: a routine
# `nix-collect-garbage` deletes it and the cache dies with "No such file or
# directory" until someone rebuilds it. Root it under the user running setup
# and point $xzar_dir/server-bin at it, so the path the service command uses
# survives GC and a rebuild only needs setup.sh re-run.
echo "== xzar binary GC root"
mkdir -p "$(dirname "$xzar_bin_root")"
nix-store --add-root "$xzar_bin_root" --indirect -r "$XZAR_SERVER_BIN" >/dev/null
ln -sfn "$(readlink -f "$XZAR_SERVER_BIN")" "$xzar_dir/server-bin"
echo "  $xzar_dir/server-bin -> $(readlink "$xzar_dir/server-bin")"

echo "== incus client certificate"
if [ ! -s "$runner_dir/incus-client.crt" ]; then
  openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "$runner_dir/incus-client.key" -out "$runner_dir/incus-client.crt" \
    -days 3650 -nodes -subj "/CN=mac-mgmt-runner@$(hostname)" 2>/dev/null
fi
chmod 600 "$runner_dir/incus-client.key"

incus "incus config trust list --format csv -c n" | grep -qx 'reagent-runner' || {
  cat "$runner_dir/incus-client.crt" | incus "cat > /tmp/runner-client.crt && \
    incus config trust add-certificate /tmp/runner-client.crt --name reagent-runner"
}

echo "== databases"
for db in macmgmt xzar; do
  psql -lqt | cut -d'|' -f1 | grep -qw "$db" || createdb "$db"
done

echo "== xzar signing key"
if [ ! -s "$xzar_dir/signing-key.pem" ]; then
  nix-store --generate-binary-cache-key "$(hostname)-1" \
    "$xzar_dir/signing-key.pem" "$xzar_dir/signing-pub-key.pem" 2>/dev/null
  chmod 600 "$xzar_dir/signing-key.pem"
fi
echo "nix public key: $(cat "$xzar_dir/signing-pub-key.pem")"

echo "== xzar api token"
if [ ! -s "$xzar_dir/api.token" ]; then
  XZAR_STORAGE="$xzar_dir/storage" DATABASE_URL="$XZAR_DSN" \
    "$XZAR_SERVER_BIN" --config "$xzar_dir/config.yaml" \
    token create --system --read --write --description "fleet" 2>/dev/null |
    sed -n 's/^Token: //p' > "$xzar_dir/api.token"
  chmod 600 "$xzar_dir/api.token"
fi

echo "== seed the server database (creates the admin token and organization)"
"$_here/seed.sh"
organization_id=$(psql -d macmgmt -tAc "select id from organizations where name='Reagent Fleet'")
admin_token=$(cat "$admin_token_file")

echo "== configs (render the .env values into them)"
render() {
  src="$1"; dst="$2"
  if [ -f "$dst" ]; then echo "  keep existing $dst"; return; fi
  sed -e "s|@SERVER_PORT@|$SERVER_PORT|g" \
      -e "s|@SERVER_EXTERNAL_URL@|$SERVER_EXTERNAL_URL|g" \
      -e "s|@INSTANCE_SERVER_URL@|${INSTANCE_SERVER_URL:-$SERVER_EXTERNAL_URL}|g" \
      -e "s|@RUNNER_PORT@|$RUNNER_PORT|g" \
      -e "s|@INSTANCE_PREFIX@|$INSTANCE_PREFIX|g" \
      -e "s|@ADMIN_TOKEN@|$admin_token|g" \
      -e "s|@ORGANIZATION_ID@|$organization_id|g" \
      -e "s|@COOKIE_SECRET@|$(openssl rand -hex 32)|g" \
      -e "s|@XZAR_PORT@|$XZAR_PORT|g" \
      -e "s|@XZAR_URL@|$XZAR_URL|g" \
      -e "s|@MACMGMT_DSN@|$MACMGMT_DSN|g" \
      -e "s|@XZAR_DSN@|$XZAR_DSN|g" \
      -e "s|@FLEET_DIR@|$fleet_dir|g" \
      -e "s|@XZAR_SIGNING_KEY@|$xzar_dir/signing-key.pem|g" \
      -e "s|@XZAR_SIGNING_PUB_KEY@|$xzar_dir/signing-pub-key.pem|g" \
      -e "s|@INCUS_URL@|$INCUS_URL|g" \
      -e "s|@INCUS_PROJECT@|$INCUS_PROJECT|g" \
      -e "s|@INCUS_CLIENT_CERT@|$INCUS_CLIENT_CERT|g" \
      -e "s|@INCUS_CLIENT_KEY@|$INCUS_CLIENT_KEY|g" \
      -e "s|@DAEMON_SYSTEM@|$DAEMON_SYSTEM|g" \
      -e "s|@XZAR_TOKEN@|$(cat "$xzar_dir/api.token")|g" \
      -e "s|@XZAR_PUBLIC_KEY@|$(cat "$xzar_dir/signing-pub-key.pem")|g" \
      "$src" > "$dst"
  chmod 600 "$dst"
  echo "  wrote $dst"
}
render "$_here/templates/server-config.toml" "$server_dir/config.toml"
render "$_here/templates/runner-config.toml" "$runner_dir/config.toml"
render "$_here/templates/xzar-config.yaml" "$xzar_dir/config.yaml"
