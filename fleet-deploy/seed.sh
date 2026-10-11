#!/bin/sh
# Idempotent seed for the local mac-mgmt server: the admin token, the
# organization the fleet runs in, and the daemon version that cloud-init
# resolves when no rollout or pin is set. Run after the server has migrated.
set -eu
. "$(dirname "$0")/lib.sh"

db=macmgmt
psql_db() { psql -d "$db" "$@"; }

if [ ! -s "$admin_token_file" ]; then
  openssl rand -hex 32 > "$admin_token_file"
  chmod 600 "$admin_token_file"
  echo "created admin token: $admin_token_file"
fi

admin_hash=$(sha256sum < "$admin_token_file" | cut -d' ' -f1)
psql_db -qtAc "INSERT INTO tokens (cluster_id, token_hash, label, kind)
  SELECT NULL, '$admin_hash', 'fleet-admin', 'admin'
  WHERE NOT EXISTS (SELECT 1 FROM tokens WHERE token_hash = '$admin_hash')" >/dev/null

# The runner's organization. It creates every matrix cluster under this one.
psql_db -qtAc "INSERT INTO organizations (id, name)
  SELECT gen_random_uuid(), 'Reagent Fleet'
  WHERE NOT EXISTS (SELECT 1 FROM organizations WHERE name = 'Reagent Fleet')" >/dev/null

# Without a daemon_versions row (and no rollout/pin) cloud-init cannot resolve
# a version and answers 422.
psql_db -qtAc "INSERT INTO daemon_versions (version) VALUES ('$DAEMON_VERSION')
  ON CONFLICT (version) DO NOTHING" >/dev/null

echo "organization:  $(psql_db -tAc "select id from organizations where name='Reagent Fleet'")"
echo "daemon_versions: $(psql_db -tAc 'select string_agg(version, chr(44)) from daemon_versions')"
echo "admin token:   $admin_token_file"
