CREATE TABLE daemon_heartbeats (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id   UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    instance_id   TEXT NOT NULL,
    version       TEXT NOT NULL,
    services      JSONB NOT NULL DEFAULT '[]',
    reported_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(customer_id, instance_id)
);
