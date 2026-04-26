-- Add optional name to rollouts for human-friendly identification.
ALTER TABLE rollouts ADD COLUMN IF NOT EXISTS name TEXT;
