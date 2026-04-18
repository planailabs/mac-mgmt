ALTER TABLE daemon_heartbeats ADD COLUMN shell_tunnels JSONB NOT NULL DEFAULT '[]'::jsonb;
