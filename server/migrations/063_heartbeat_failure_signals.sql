-- Host-level failure signals (drift/disk/load/thermal) piggybacked on heartbeats.
ALTER TABLE daemon_heartbeats ADD COLUMN failure_signals JSONB;
