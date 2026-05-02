-- Add explicit enabled=true to existing provider sections that lack the key.
--
-- The default for ollama/lms/openclaw changed from true to false.
-- Existing configs that have these sections but no explicit `enabled` field
-- were relying on the old default. Make it explicit so they keep working.

UPDATE cluster_configs
SET config_json = jsonb_set(config_json, '{ollama,enabled}', 'true')
WHERE config_json ? 'ollama'
  AND NOT (config_json->'ollama' ? 'enabled');

UPDATE cluster_configs
SET config_json = jsonb_set(config_json, '{lms,enabled}', 'true')
WHERE config_json ? 'lms'
  AND NOT (config_json->'lms' ? 'enabled');

UPDATE cluster_configs
SET config_json = jsonb_set(config_json, '{openclaw,enabled}', 'true')
WHERE config_json ? 'openclaw'
  AND NOT (config_json->'openclaw' ? 'enabled');
