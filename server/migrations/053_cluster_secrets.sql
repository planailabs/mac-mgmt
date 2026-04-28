CREATE TABLE IF NOT EXISTS cluster_secrets (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id      UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    encrypted_value BYTEA NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cluster_id, name)
);
CREATE INDEX IF NOT EXISTS idx_cluster_secrets_cluster ON cluster_secrets(cluster_id);
