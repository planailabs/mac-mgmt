-- Store the single shared ACME account as a credential row. Creating a fresh
-- account per issuance trips Let's Encrypt's new-registration rate limit, so
-- hosts with several domain bindings would fail on the later domains.
ALTER TABLE credentials
  DROP CONSTRAINT credentials_credential_type_check,
  ADD CONSTRAINT credentials_credential_type_check
    CHECK (credential_type IN ('cloudflare', 'spaceship', 'mac-mgmt', 'changedetection', 'acme-account'));

-- The old per-certificate column was never used; the account is global.
ALTER TABLE certificates DROP COLUMN encrypted_acme_account;
