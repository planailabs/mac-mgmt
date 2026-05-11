CREATE TABLE reachability_results (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_id     UUID NOT NULL REFERENCES webspaces(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    hostname        TEXT NOT NULL,
    http_ok         BOOLEAN NOT NULL DEFAULT false,
    ssl_ok          BOOLEAN NOT NULL DEFAULT false,
    proxy_ok        BOOLEAN NOT NULL DEFAULT false,
    latency_ms      INTEGER,
    error_message   TEXT,
    checked_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_reachability_hostname ON reachability_results(hostname);
CREATE INDEX idx_reachability_org ON reachability_results(organization_id);
CREATE INDEX idx_reachability_webspace ON reachability_results(webspace_id);
