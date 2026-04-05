-- Rollouts are about daemon versions.
-- Each customer tracks their pinned version.

ALTER TABLE customers ADD COLUMN pinned_version TEXT;

-- Change rollouts from config delivery to version rollouts
ALTER TABLE rollouts DROP COLUMN config_toml;
ALTER TABLE rollouts ADD COLUMN target_version TEXT NOT NULL;
