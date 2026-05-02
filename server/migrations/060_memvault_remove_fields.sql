-- Remove deprecated memvault config fields (auth_token_hash, web_enabled).
-- Auth token is now auto-generated on disk; web_enabled was never wired up.

UPDATE cluster_configs
SET    config_json = config_json #- '{memvault,auth_token_hash}'
WHERE  config_json #> '{memvault,auth_token_hash}' IS NOT NULL;

UPDATE cluster_configs
SET    config_json = config_json #- '{memvault,web_enabled}'
WHERE  config_json #> '{memvault,web_enabled}' IS NOT NULL;
