-- Add 'relay' hosting type and 'mac-mgmt' credential type.

ALTER TABLE webspaces
  DROP CONSTRAINT webspaces_hosting_type_check,
  ADD CONSTRAINT webspaces_hosting_type_check
    CHECK (hosting_type IN ('cloudflare_pages', 'local', 'relay'));

ALTER TABLE webspaces
  ADD COLUMN relay_url TEXT,
  ADD COLUMN relay_credential_id UUID REFERENCES credentials(id);

ALTER TABLE credentials
  DROP CONSTRAINT credentials_credential_type_check,
  ADD CONSTRAINT credentials_credential_type_check
    CHECK (credential_type IN ('cloudflare', 'spaceship', 'mac-mgmt'));
