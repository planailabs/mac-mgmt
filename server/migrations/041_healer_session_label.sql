-- Human-readable session label (set by the LLM or auto-trigger).
ALTER TABLE healer_sessions ADD COLUMN label TEXT;
