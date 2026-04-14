-- Attach the heartbeat-piggybacked dynamic sample + rolled-up extended service
-- state directly to the latest heartbeat row. Small, always-present data.
ALTER TABLE daemon_heartbeats ADD COLUMN IF NOT EXISTS sample JSONB;
ALTER TABLE daemon_heartbeats ADD COLUMN IF NOT EXISTS services_extended JSONB;

-- Full static inventory + security-posture snapshots. Produced on startup,
-- every ~6h, or on RequestAssessment push. Retained indefinitely but tooling
-- will prune to the last N per instance + one per day older than 10 days.
CREATE TABLE assessments (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id    UUID        NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    instance_id   TEXT        NOT NULL,
    collected_at  TIMESTAMPTZ NOT NULL,
    inventory     JSONB       NOT NULL,
    security      JSONB       NOT NULL,
    received_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX assessments_cluster_instance_idx
    ON assessments (cluster_id, instance_id, collected_at DESC);

-- Per-probe functional-check results (full prompt round-trips for LLM backends,
-- tool invocations for MCP servers, dry-run for apprise, etc.). Each run emits
-- one row per probe. Used as a rollout gate datasource.
CREATE TABLE assessment_probes (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id     UUID        NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    instance_id    TEXT        NOT NULL,
    service        TEXT        NOT NULL,
    kind           TEXT        NOT NULL,
    ok             BOOLEAN     NOT NULL,
    duration_ms    BIGINT      NOT NULL,
    tokens_in      INTEGER,
    tokens_out     INTEGER,
    first_token_ms BIGINT,
    model          TEXT,
    canary_digest  TEXT,
    error_class    TEXT,
    error_detail   TEXT,
    collected_at   TIMESTAMPTZ NOT NULL,
    received_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX assessment_probes_cluster_service_idx
    ON assessment_probes (cluster_id, instance_id, service, collected_at DESC);
CREATE INDEX assessment_probes_collected_at_idx
    ON assessment_probes (collected_at DESC);
