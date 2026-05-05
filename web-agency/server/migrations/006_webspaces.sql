CREATE TABLE webspaces (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name                     TEXT NOT NULL,
    hosting_type             TEXT NOT NULL DEFAULT 'cloudflare_pages' CHECK (hosting_type IN ('cloudflare_pages', 'local')),
    -- Cloudflare Pages fields (hosting_type = 'cloudflare_pages')
    cloudflare_pages_project TEXT,
    cloudflare_credential_id UUID REFERENCES credentials(id) ON DELETE SET NULL,
    -- Local hosting fields (hosting_type = 'local')
    runtime                  TEXT CHECK (runtime IN ('static', 'nodejs', 'docker')),
    runtime_config           JSONB,
    local_status             TEXT DEFAULT 'stopped' CHECK (local_status IN ('stopped', 'starting', 'running', 'error')),
    local_port               INTEGER,
    --
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

CREATE INDEX idx_webspaces_org ON webspaces(organization_id);
