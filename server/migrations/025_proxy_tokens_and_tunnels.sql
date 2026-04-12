-- Add tunnels column to daemon_heartbeats for exposed TCP tunnels.
ALTER TABLE daemon_heartbeats ADD COLUMN IF NOT EXISTS tunnels JSONB NOT NULL DEFAULT '[]';

-- Add expires_at to tokens for short-lived proxy tokens.
ALTER TABLE tokens ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
