-- Rollouts are about daemon versions, not configs.
-- Each customer has an environment (update channel) and a pinned version.
-- A rollout targets a specific version and moves groups of customers to it.

ALTER TABLE customers ADD COLUMN environment TEXT NOT NULL DEFAULT 'stable';
ALTER TABLE customers ADD COLUMN pinned_version TEXT;

-- Change rollouts from config delivery to version rollouts
ALTER TABLE rollouts DROP COLUMN config_toml;
ALTER TABLE rollouts ADD COLUMN target_version TEXT NOT NULL;
ALTER TABLE rollouts ADD COLUMN target_environment TEXT NOT NULL DEFAULT 'stable';
