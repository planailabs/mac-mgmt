-- Add a per-cluster pre-shared key for libp2p peer authentication.
-- The server generates a random 32-byte PSK per cluster when NULL
-- and distributes it via ClusterConfig.relay.cluster_psk.
ALTER TABLE clusters ADD COLUMN IF NOT EXISTS p2p_psk BYTEA;
