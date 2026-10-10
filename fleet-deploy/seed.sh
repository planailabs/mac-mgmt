#!/bin/sh
# Idempotent seed for the local mac-mgmt server: the admin token, the
# organization the fleet runs in, and the daemon version that cloud-init
# resolves when no rollout/pin is set.
set -eu
DB=macmgmt

# Admin token — printed once; also written to the fleet directory.
if [ ! -s /var/lib/reagent/fleet/admin.token ]; then
  openssl rand -hex 32 > /var/lib/reagent/fleet/admin.token
  chmod 600 /var/lib/reagent/fleet/admin.token
fi
ADMIN_HASH=$(sha256sum < /var/lib/reagent/fleet/admin.token | cut -d' ' -f1)
psql -d "$DB" -qtAc "INSERT INTO tokens (cluster_id, token_hash, label, kind)
  SELECT NULL, '$ADMIN_HASH', 'fleet-admin', 'admin'
  WHERE NOT EXISTS (SELECT 1 FROM tokens WHERE token_hash = '$ADMIN_HASH')" >/dev/null

psql -d "$DB" -qtAc "INSERT INTO organizations (id, name)
  SELECT gen_random_uuid(), 'Reagent Fleet'
  WHERE NOT EXISTS (SELECT 1 FROM organizations WHERE name = 'Reagent Fleet')" >/dev/null

# The daemon version cloud-init hands to new instances. Must match the pin
# published in the local xzar (daemon/<version>/x86_64-linux).
psql -d "$DB" -qtAc "INSERT INTO daemon_versions (version) VALUES ('0.1.5')
  ON CONFLICT (version) DO NOTHING" >/dev/null

echo "organization: $(psql -d "$DB" -tAc "select id from organizations where name='Reagent Fleet'")"
echo "daemon_versions: $(psql -d "$DB" -tAc 'select string_agg(version, chr(44)) from daemon_versions')"
