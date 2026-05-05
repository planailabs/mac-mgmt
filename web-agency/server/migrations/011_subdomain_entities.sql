-- Refactor subdomains from "a subdomain IS a record" to "a subdomain HAS records".
-- Subdomains become proper entities (e.g. "www", "@", "api") that can be bound to
-- webspaces and have multiple DNS records attached.

-- 1. Create dns_records table
CREATE TABLE dns_records (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subdomain_id         UUID,
    domain_id            UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name                 TEXT NOT NULL,
    record_type          TEXT NOT NULL,
    record_value         TEXT NOT NULL,
    ttl                  INTEGER DEFAULT 1,
    proxied              BOOLEAN NOT NULL DEFAULT true,
    cloudflare_record_id TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2. Copy existing records from subdomains into dns_records
INSERT INTO dns_records (domain_id, name, record_type, record_value, proxied, cloudflare_record_id, created_at, updated_at)
SELECT domain_id, name, record_type, record_value, proxied, cloudflare_record_id, created_at, updated_at
FROM subdomains;

-- 3. Rebuild subdomains as entity table
DELETE FROM subdomains;
ALTER TABLE subdomains DROP CONSTRAINT IF EXISTS subdomains_domain_id_name_record_type_key;
ALTER TABLE subdomains DROP COLUMN IF EXISTS record_type;
ALTER TABLE subdomains DROP COLUMN IF EXISTS record_value;
ALTER TABLE subdomains DROP COLUMN IF EXISTS proxied;
ALTER TABLE subdomains DROP COLUMN IF EXISTS cloudflare_record_id;
ALTER TABLE subdomains ADD CONSTRAINT subdomains_domain_id_name_key UNIQUE (domain_id, name);

-- 4. Insert unique subdomain entities from migrated records
INSERT INTO subdomains (domain_id, name)
SELECT DISTINCT domain_id, name FROM dns_records
ON CONFLICT DO NOTHING;

-- 5. Link dns_records to subdomain entities
UPDATE dns_records dr SET subdomain_id = s.id
FROM subdomains s
WHERE s.domain_id = dr.domain_id AND s.name = dr.name;

-- 6. Make subdomain_id NOT NULL and add FK
ALTER TABLE dns_records ALTER COLUMN subdomain_id SET NOT NULL;
ALTER TABLE dns_records ADD CONSTRAINT dns_records_subdomain_fk
    FOREIGN KEY (subdomain_id) REFERENCES subdomains(id) ON DELETE CASCADE;

CREATE INDEX idx_dns_records_subdomain ON dns_records(subdomain_id);
CREATE INDEX idx_dns_records_domain ON dns_records(domain_id);
