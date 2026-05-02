-- Remove deprecated memvault config fields (auth_token_hash, web_enabled).
-- Auth token is now auto-generated on disk; web_enabled was never wired up.
UPDATE cluster_configs
SET    config = json_remove(json_remove(config,
           '$.memvault.auth_token_hash'),
           '$.memvault.web_enabled')
WHERE  json_extract(config, '$.memvault.auth_token_hash') IS NOT NULL
    OR json_extract(config, '$.memvault.web_enabled') IS NOT NULL;
