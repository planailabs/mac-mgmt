-- Healer agent sessions and conversation logs.

CREATE TABLE healer_sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id      UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    instance_id     TEXT NOT NULL,
    state           TEXT NOT NULL DEFAULT 'created',
    state_data      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by      TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ,
    error_message   TEXT,
    initial_issues  JSONB NOT NULL DEFAULT '[]'::jsonb
);

CREATE INDEX idx_healer_sessions_cluster
    ON healer_sessions (cluster_id, created_at DESC);

-- Sessions that need auto-resume on server startup (excludes terminal + paused).
CREATE INDEX idx_healer_sessions_resumable
    ON healer_sessions (state)
    WHERE state NOT IN ('completed', 'failed', 'cancelled', 'paused');

CREATE TABLE healer_messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id  UUID NOT NULL REFERENCES healer_sessions(id) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    metadata    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_healer_messages_session
    ON healer_messages (session_id, created_at ASC);
