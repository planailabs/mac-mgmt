CREATE TABLE domains (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name                     TEXT NOT NULL,
    registrar_type           TEXT CHECK (registrar_type IN ('cloudflare', 'spaceship', 'external')),
    registrar_credential_id  UUID REFERENCES credentials(id) ON DELETE SET NULL,
    cloudflare_credential_id UUID REFERENCES credentials(id) ON DELETE SET NULL,
    cloudflare_zone_id       TEXT,
    ssl_mode                 TEXT NOT NULL DEFAULT 'full' CHECK (ssl_mode IN ('off', 'flexible', 'full', 'strict')),
    dnssec_enabled           BOOLEAN NOT NULL DEFAULT false,
    registered_at            TIMESTAMPTZ,
    expires_at               TIMESTAMPTZ,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

CREATE INDEX idx_domains_org ON domains(organization_id);
