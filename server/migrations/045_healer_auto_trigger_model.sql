-- Per-cluster auto-trigger provider/model overrides.
ALTER TABLE healer_cluster_settings ADD COLUMN IF NOT EXISTS auto_trigger_provider TEXT;
ALTER TABLE healer_cluster_settings ADD COLUMN IF NOT EXISTS auto_trigger_model TEXT;
