-- Per-service dynamic samples piggybacked on heartbeats.
ALTER TABLE daemon_heartbeats ADD COLUMN service_samples JSONB;
