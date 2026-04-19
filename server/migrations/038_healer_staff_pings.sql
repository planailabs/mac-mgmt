-- Staff pings: actionable notifications from the healer agent to admins.

CREATE TABLE healer_staff_pings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      UUID NOT NULL REFERENCES healer_sessions(id) ON DELETE CASCADE,
    cluster_id      UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    instance_id     TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT 'other',
    message         TEXT NOT NULL,
    resolved        BOOLEAN NOT NULL DEFAULT false,
    resolved_by     TEXT,
    resolved_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_staff_pings_cluster
    ON healer_staff_pings (cluster_id, resolved, created_at DESC);

CREATE INDEX idx_staff_pings_session
    ON healer_staff_pings (session_id, created_at ASC);

-- Update partial index to include new states.
DROP INDEX IF EXISTS idx_healer_sessions_resumable;
CREATE INDEX idx_healer_sessions_resumable
    ON healer_sessions (state)
    WHERE state NOT IN ('completed', 'failed', 'cancelled', 'paused', 'success', 'needs_human_attention');
