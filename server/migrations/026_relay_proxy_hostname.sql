-- Relay proxy hostname reported by the daemon (e.g. "relay.plan.ai").
ALTER TABLE daemon_heartbeats ADD COLUMN IF NOT EXISTS relay_proxy_hostname TEXT;
