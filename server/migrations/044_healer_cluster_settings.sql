-- Per-cluster healer settings (separate from the daemon ClusterConfig).
-- Server-only: controls auto-trigger, approval, and fix-model behavior.
CREATE TABLE IF NOT EXISTS healer_cluster_settings (
    cluster_id   UUID PRIMARY KEY REFERENCES clusters(id) ON DELETE CASCADE,
    auto_trigger BOOLEAN,
    auto_approve BOOLEAN,
    fix_provider TEXT,
    fix_model    TEXT,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
