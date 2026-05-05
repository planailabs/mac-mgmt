CREATE TABLE tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    label           TEXT NOT NULL DEFAULT '',
    kind            TEXT NOT NULL DEFAULT 'api',
    scopes          JSONB,
    revoked         BOOLEAN NOT NULL DEFAULT false,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
