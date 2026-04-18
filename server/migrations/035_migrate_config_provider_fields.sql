-- Migrate existing cluster configs to the new provider field names and cloud list format.
--
-- 1. Rename global.llm_provider → global.default_llm
-- 2. Rename global.agent_provider → global.default_agent
-- 3. Wrap bare cloud objects into a single-element array

-- Step 1 & 2: Rename global fields (only where old keys exist)
UPDATE cluster_configs
SET config_json = jsonb_set(
    config_json #- '{global,llm_provider}',
    '{global,default_llm}',
    config_json->'global'->'llm_provider'
)
WHERE config_json->'global' ? 'llm_provider'
  AND NOT (config_json->'global' ? 'default_llm');

UPDATE cluster_configs
SET config_json = jsonb_set(
    config_json #- '{global,agent_provider}',
    '{global,default_agent}',
    config_json->'global'->'agent_provider'
)
WHERE config_json->'global' ? 'agent_provider'
  AND NOT (config_json->'global' ? 'default_agent');

-- Step 3: Wrap cloud object into array (where cloud is an object, not array)
UPDATE cluster_configs
SET config_json = jsonb_set(
    config_json,
    '{cloud}',
    jsonb_build_array(config_json->'cloud')
)
WHERE jsonb_typeof(config_json->'cloud') = 'object';
