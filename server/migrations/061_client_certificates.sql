-- Client TLS certificates for relay authentication.
-- cluster_client_certs: per-cluster certificates.
-- admin_client_certs: global admin certificates.

CREATE TABLE IF NOT EXISTS cluster_client_certs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id    UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    fingerprint   TEXT NOT NULL,
    label         TEXT NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cluster_id, fingerprint)
);

CREATE TABLE IF NOT EXISTS admin_client_certs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint   TEXT NOT NULL UNIQUE,
    label         TEXT NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
