-- Add hide_from_public_catalog flag to skills, bundles, mcp_servers and
-- mcp_server_bundles. When true, the entity is omitted from the public
-- /api/setting/available/* listings and /api/setting/catalog endpoint.
ALTER TABLE skills              ADD COLUMN hide_from_public_catalog BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE bundles             ADD COLUMN hide_from_public_catalog BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE mcp_servers         ADD COLUMN hide_from_public_catalog BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE mcp_server_bundles  ADD COLUMN hide_from_public_catalog BOOLEAN NOT NULL DEFAULT false;
