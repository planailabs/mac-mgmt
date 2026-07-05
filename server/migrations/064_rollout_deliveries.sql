-- Track which clusters have fetched a rollout's target. Once delivered, a
-- cluster keeps getting that rollout's version/nixpkgs for the rollout's
-- lifetime (rolling or paused), so pausing/gating mid-rollout never
-- downgrades a cluster that already updated.
CREATE TABLE rollout_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rollout_id UUID NOT NULL REFERENCES rollouts(id) ON DELETE CASCADE,
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    delivered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (rollout_id, cluster_id)
);

CREATE INDEX rollout_deliveries_cluster_idx ON rollout_deliveries (cluster_id);
