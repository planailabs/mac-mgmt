-- Unified client_certificates table replacing admin_client_certs + cluster_client_certs.
-- Supports admin, organization, and cluster scopes, plus CA certificates.

CREATE TABLE client_certificates (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope           TEXT NOT NULL CHECK (scope IN ('admin', 'organization', 'cluster')),
    scope_id        UUID,           -- NULL for admin; org_id or cluster_id otherwise
    is_ca           BOOLEAN NOT NULL DEFAULT false,
    fingerprint     TEXT NOT NULL,
    certificate_pem TEXT,           -- required for CAs, optional for direct certs (backfilled)
    label           TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Uniqueness: one fingerprint per (scope, scope_id, is_ca) combination.
-- COALESCE handles NULL scope_id for admin scope.
CREATE UNIQUE INDEX uq_client_certificates
    ON client_certificates (scope, COALESCE(scope_id, '00000000-0000-0000-0000-000000000000'), fingerprint, is_ca);

CREATE INDEX idx_cc_fingerprint ON client_certificates (fingerprint);
CREATE INDEX idx_cc_scope_id    ON client_certificates (scope_id) WHERE scope_id IS NOT NULL;
CREATE INDEX idx_cc_is_ca       ON client_certificates (is_ca) WHERE is_ca = true;

-- Migrate existing data.
INSERT INTO client_certificates (id, scope, scope_id, is_ca, fingerprint, label, created_at)
    SELECT id, 'admin', NULL, false, fingerprint, label, created_at
    FROM admin_client_certs;

INSERT INTO client_certificates (id, scope, scope_id, is_ca, fingerprint, label, created_at)
    SELECT id, 'cluster', cluster_id, false, fingerprint, label, created_at
    FROM cluster_client_certs;

-- Drop old tables.
DROP TABLE admin_client_certs;
DROP TABLE cluster_client_certs;

-- Cleanup trigger: remove certificates when a cluster or organization is deleted.
CREATE OR REPLACE FUNCTION cleanup_client_certificates() RETURNS trigger AS $$
BEGIN
    DELETE FROM client_certificates
    WHERE scope_id = OLD.id
      AND scope = TG_ARGV[0];
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_cleanup_cluster_certs
    AFTER DELETE ON clusters
    FOR EACH ROW EXECUTE FUNCTION cleanup_client_certificates('cluster');

CREATE TRIGGER trg_cleanup_org_certs
    AFTER DELETE ON organizations
    FOR EACH ROW EXECUTE FUNCTION cleanup_client_certificates('organization');
