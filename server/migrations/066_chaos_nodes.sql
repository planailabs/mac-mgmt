-- Chaos nodes are disposable test instances spawned by the mmrc chaos harness.
-- They enroll and heartbeat like normal daemons but must never count toward
-- fleet/rollout health. A daemon's instance_id is registered here (by the
-- gated chaos API) before it first heartbeats; post_heartbeat then stamps the
-- matching heartbeat row with chaos=true.
CREATE TABLE chaos_nodes (
    cluster_id  UUID        NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    instance_id TEXT        NOT NULL,
    label       TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (cluster_id, instance_id)
);

CREATE INDEX chaos_nodes_instance_idx ON chaos_nodes (instance_id);

ALTER TABLE daemon_heartbeats ADD COLUMN chaos BOOLEAN NOT NULL DEFAULT FALSE;
