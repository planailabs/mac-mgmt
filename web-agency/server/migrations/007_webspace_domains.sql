CREATE TABLE webspace_domains (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_id  UUID NOT NULL REFERENCES webspaces(id) ON DELETE CASCADE,
    domain_id    UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    subdomain_id UUID REFERENCES subdomains(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (webspace_id, domain_id, subdomain_id)
);

CREATE INDEX idx_webspace_domains_webspace ON webspace_domains(webspace_id);
CREATE INDEX idx_webspace_domains_domain ON webspace_domains(domain_id);
