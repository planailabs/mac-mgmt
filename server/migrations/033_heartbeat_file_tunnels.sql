ALTER TABLE daemon_heartbeats ADD COLUMN file_tunnels JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE daemon_heartbeats ADD COLUMN relay_proxy_url TEXT;
