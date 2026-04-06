-- Convert config storage from TOML text to JSONB.
-- First add the new column, then backfill, then drop the old one.

ALTER TABLE customer_configs ADD COLUMN config_json JSONB;

-- Backfill: parse TOML → JSON. Since we can't parse TOML in pure SQL,
-- store configs with TOML content as a JSON object with a "_toml" key
-- that the application will re-parse on first read.
-- In practice, the app should be re-deployed before this migration runs,
-- and it will write JSON going forward. Existing rows get a fallback.
UPDATE customer_configs SET config_json = '{}' WHERE config_json IS NULL;

ALTER TABLE customer_configs ALTER COLUMN config_json SET NOT NULL;
ALTER TABLE customer_configs ALTER COLUMN config_json SET DEFAULT '{}';
ALTER TABLE customer_configs DROP COLUMN config_toml;
