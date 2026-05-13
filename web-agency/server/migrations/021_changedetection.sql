-- Add 'changedetection' credential type and per-webspace/org-default columns.

ALTER TABLE credentials
  DROP CONSTRAINT credentials_credential_type_check,
  ADD CONSTRAINT credentials_credential_type_check
    CHECK (credential_type IN ('cloudflare', 'spaceship', 'mac-mgmt', 'changedetection'));

-- Per-webspace changedetection credential (optional, user-assigned).
ALTER TABLE webspaces
  ADD COLUMN changedetection_credential_id UUID REFERENCES credentials(id);

-- Org-level default (used only to pre-fill the webspace creation form).
ALTER TABLE organizations
  ADD COLUMN default_changedetection_credential_id UUID REFERENCES credentials(id);
