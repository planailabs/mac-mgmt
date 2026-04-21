-- Track which LLM provider and model each healer session uses.
-- Existing rows get NULL (unknown — created before this migration).
ALTER TABLE healer_sessions ADD COLUMN provider TEXT;
ALTER TABLE healer_sessions ADD COLUMN model TEXT;
