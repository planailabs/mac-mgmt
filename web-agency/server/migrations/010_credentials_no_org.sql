-- Make credentials org-independent: drop the NOT NULL constraint and
-- the unique-per-org name constraint. Credentials are now global and
-- can be shared across organizations.

ALTER TABLE credentials ALTER COLUMN organization_id DROP NOT NULL;
DROP INDEX IF EXISTS idx_credentials_org;
ALTER TABLE credentials DROP CONSTRAINT IF EXISTS credentials_organization_id_name_key;
ALTER TABLE credentials ADD CONSTRAINT credentials_name_key UNIQUE (name);
