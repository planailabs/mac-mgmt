-- Add scopes to proxy tokens for fine-grained access control.
-- NULL means wildcard (all scopes) for backwards compatibility with existing tokens.
-- Example values: '["files:read","files:write"]', '["tcp:*"]', '["logs:read"]'
ALTER TABLE tokens ADD COLUMN IF NOT EXISTS scopes JSONB;
