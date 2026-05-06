-- Cache AI bot protection status on domains for filtering.
ALTER TABLE domains ADD COLUMN ai_bots_protection TEXT;

-- Store CF Pages project ID on webspaces for reliable dedup during import.
ALTER TABLE webspaces ADD COLUMN cloudflare_pages_project_id TEXT;
