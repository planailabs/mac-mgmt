-- Rename 'success' state to 'done' in healer sessions.
UPDATE healer_sessions SET state = 'done' WHERE state = 'success';

-- Rebuild partial index with 'done' instead of 'success'.
DROP INDEX IF EXISTS idx_healer_sessions_resumable;
CREATE INDEX idx_healer_sessions_resumable
    ON healer_sessions (state)
    WHERE state NOT IN ('completed', 'done', 'failed', 'cancelled', 'paused', 'needs_human_attention');
