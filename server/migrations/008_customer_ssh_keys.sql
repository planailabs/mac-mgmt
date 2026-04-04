CREATE TABLE customer_ssh_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    public_key  TEXT NOT NULL,
    comment     TEXT NOT NULL DEFAULT '',
    fingerprint TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(customer_id, fingerprint)
);
