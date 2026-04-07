-- Daemon versions known to the server, populated by syncing xzar pins
-- (`daemon/{version}/{system}`). Store paths are resolved live from xzar
-- on each /api/update request, mirroring how skills work.

CREATE TABLE daemon_versions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version     TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
