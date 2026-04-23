-- Append-only token usage event log for healer sessions.
-- Each row records one LLM completion's token consumption.
CREATE TABLE IF NOT EXISTS healer_token_events (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id    UUID NOT NULL REFERENCES healer_sessions(id) ON DELETE CASCADE,
    provider      TEXT NOT NULL,
    model         TEXT NOT NULL,
    input_tokens  INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_healer_token_events_session
    ON healer_token_events(session_id);

-- Denormalized budget/usage on the session row for fast budget checks.
-- tokens_used is updated atomically on each event insert.
ALTER TABLE healer_sessions ADD COLUMN IF NOT EXISTS token_budget BIGINT NOT NULL DEFAULT 0;
ALTER TABLE healer_sessions ADD COLUMN IF NOT EXISTS tokens_used BIGINT NOT NULL DEFAULT 0;
