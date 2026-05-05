CREATE TABLE billing_entries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    entry_type      TEXT NOT NULL CHECK (entry_type IN ('domain_registration', 'domain_renewal', 'webspace_hosting')),
    description     TEXT NOT NULL DEFAULT '',
    amount_cents    INTEGER NOT NULL,
    currency        TEXT NOT NULL DEFAULT 'EUR',
    domain_id       UUID REFERENCES domains(id) ON DELETE SET NULL,
    webspace_id     UUID REFERENCES webspaces(id) ON DELETE SET NULL,
    period_start    DATE,
    period_end      DATE,
    provider        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_billing_org ON billing_entries(organization_id);
CREATE INDEX idx_billing_domain ON billing_entries(domain_id);
CREATE INDEX idx_billing_webspace ON billing_entries(webspace_id);

CREATE TABLE domain_pricing (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tld                      TEXT NOT NULL,
    registrar_type           TEXT NOT NULL CHECK (registrar_type IN ('cloudflare', 'spaceship')),
    registration_price_cents INTEGER NOT NULL,
    renewal_price_cents      INTEGER NOT NULL,
    currency                 TEXT NOT NULL DEFAULT 'USD',
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tld, registrar_type)
);
