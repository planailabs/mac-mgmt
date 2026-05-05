CREATE TABLE subdomains (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id            UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name                 TEXT NOT NULL,
    record_type          TEXT NOT NULL DEFAULT 'CNAME',
    record_value         TEXT NOT NULL,
    proxied              BOOLEAN NOT NULL DEFAULT true,
    cloudflare_record_id TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (domain_id, name, record_type)
);

CREATE INDEX idx_subdomains_domain ON subdomains(domain_id);
