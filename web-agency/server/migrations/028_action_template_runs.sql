-- Durable queue + history for action-template runs. Templates themselves are
-- YAML files baked into the binary; this table holds one row per execution,
-- doubling as the work queue (status queued/running) and the checkpoint
-- store (next_step/variables/steps) so interrupted runs resume after a
-- restart without re-executing completed steps.
CREATE TABLE action_template_runs (
    id              UUID PRIMARY KEY,
    template_name   TEXT NOT NULL,
    subject         TEXT NOT NULL,
    -- Authz snapshot of the caller; runs resume with the same permissions.
    principal       JSONB NOT NULL,
    params          JSONB NOT NULL DEFAULT '{}'::jsonb,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'running', 'ok', 'failed')),
    -- Checkpoint: next step to execute + variable state entering it.
    next_step       INT NOT NULL DEFAULT 0,
    variables       JSONB NOT NULL DEFAULT '{}'::jsonb,
    steps           JSONB NOT NULL DEFAULT '[]'::jsonb,
    log             JSONB NOT NULL DEFAULT '[]'::jsonb,
    report          JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_action_template_runs_claim
    ON action_template_runs (status, created_at);
CREATE INDEX idx_action_template_runs_history
    ON action_template_runs (template_name, created_at DESC);
