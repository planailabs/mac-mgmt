-- Reusable credential lists, scoped to an organization
CREATE TABLE basic_auth_lists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(organization_id, name)
);

-- Individual credentials within a list
CREATE TABLE basic_auth_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id UUID NOT NULL REFERENCES basic_auth_lists(id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(list_id, username)
);

-- Auth mode on webspaces
ALTER TABLE webspaces
  ADD COLUMN auth_mode TEXT NOT NULL DEFAULT 'none'
    CHECK (auth_mode IN ('none', 'oidc', 'basic')),
  ADD COLUMN auth_basic_list_id UUID REFERENCES basic_auth_lists(id);
