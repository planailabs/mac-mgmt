-- Per-stage health gate config, evaluated before the stage is allowed to
-- advance and continuously while rolling. NULL = no health gate (legacy).
-- Shape:
--   {
--     "min_heartbeat_fresh_pct": 95,
--     "heartbeat_freshness_secs": 180,
--     "min_probe_ok_pct": { "openclaw": 90, "ollama": 90 },
--     "grace_period_secs": 600
--   }
ALTER TABLE rollout_stages ADD COLUMN IF NOT EXISTS health_gate JSONB;

-- History of gate evaluations so the admin UI can render a timeline and
-- operators can investigate why a stage was paused.
CREATE TABLE rollout_stage_health_evaluations (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    stage_id     UUID        NOT NULL REFERENCES rollout_stages(id) ON DELETE CASCADE,
    passed       BOOLEAN     NOT NULL,
    report       JSONB       NOT NULL,
    evaluated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX rollout_stage_health_evaluations_stage_idx
    ON rollout_stage_health_evaluations (stage_id, evaluated_at DESC);
