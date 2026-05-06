ALTER TABLE webspaces
  DROP CONSTRAINT webspaces_hosting_type_check,
  ADD CONSTRAINT webspaces_hosting_type_check
    CHECK (hosting_type IN ('cloudflare_pages', 'local', 'relay', 'tunnel'));
