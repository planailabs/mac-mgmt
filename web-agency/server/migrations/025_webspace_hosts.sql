-- Introduce the webspace-host layer.
--
-- A webspace_host owns the hostname (domain bindings + CNAME flow + ChangeDetection).
-- Webspaces become path-mounted "folders" under a host:
--   * kind = 'proxy'      -> many folders (local/relay/tunnel), routed by web-agency-proxy,
--                            main-folder at '/', subfolders at subpaths. CNAME -> agency_domain.
--   * kind = 'cloudflare' -> exactly one folder (cloudflare_pages) at '/'. CNAME -> {project}.pages.dev.
--
-- Existing webspaces are auto-wrapped 1:1: one host per webspace, that webspace becomes
-- the host's main-folder, and its domain bindings + changedetection move to the host.

-- 1. The host table.
CREATE TABLE webspace_hosts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('proxy', 'cloudflare')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);
CREATE INDEX idx_webspace_hosts_org ON webspace_hosts(organization_id);

-- 2. Folders: parent host + mount path.
ALTER TABLE webspaces
    ADD COLUMN webspace_host_id UUID REFERENCES webspace_hosts(id) ON DELETE CASCADE,
    ADD COLUMN path_prefix      TEXT NOT NULL DEFAULT '/';
CREATE INDEX idx_webspaces_host ON webspaces(webspace_host_id);

-- 3. Host-scoped domain bindings (replaces webspace_domains).
CREATE TABLE webspace_host_domains (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_host_id UUID NOT NULL REFERENCES webspace_hosts(id) ON DELETE CASCADE,
    domain_id        UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    subdomain_id     UUID REFERENCES subdomains(id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (webspace_host_id, domain_id, subdomain_id)
);
CREATE INDEX idx_whd_host ON webspace_host_domains(webspace_host_id);
CREATE INDEX idx_whd_domain ON webspace_host_domains(domain_id);

-- 4. Backfill: one host per existing webspace (kind from hosting_type), wrapped as main-folder.
INSERT INTO webspace_hosts (organization_id, name, kind, created_at, updated_at)
SELECT organization_id, name,
       CASE WHEN hosting_type = 'cloudflare_pages' THEN 'cloudflare' ELSE 'proxy' END,
       created_at, updated_at
FROM webspaces;

UPDATE webspaces w
SET webspace_host_id = h.id, path_prefix = '/'
FROM webspace_hosts h
WHERE h.organization_id = w.organization_id AND h.name = w.name;

-- 5. Move domain bindings onto the host (1:1, so no conflicts).
INSERT INTO webspace_host_domains (webspace_host_id, domain_id, subdomain_id, created_at)
SELECT w.webspace_host_id, wd.domain_id, wd.subdomain_id, wd.created_at
FROM webspace_domains wd
JOIN webspaces w ON w.id = wd.webspace_id;

-- 6. Enforce one main-folder per host and unique path/name within a host.
CREATE UNIQUE INDEX uq_webspaces_main_folder ON webspaces(webspace_host_id) WHERE path_prefix = '/';
CREATE UNIQUE INDEX uq_webspaces_host_path   ON webspaces(webspace_host_id, path_prefix);
CREATE UNIQUE INDEX uq_webspaces_host_name   ON webspaces(webspace_host_id, name);

-- 7. ChangeDetection follows the hostname onto the host.
ALTER TABLE webspace_hosts ADD COLUMN changedetection_credential_id UUID REFERENCES credentials(id);
UPDATE webspace_hosts h
SET changedetection_credential_id = w.changedetection_credential_id
FROM webspaces w
WHERE w.webspace_host_id = h.id AND w.path_prefix = '/';

ALTER TABLE changedetection_suburls
    ADD COLUMN webspace_host_id UUID REFERENCES webspace_hosts(id) ON DELETE CASCADE;
UPDATE changedetection_suburls s
SET webspace_host_id = w.webspace_host_id
FROM webspaces w WHERE w.id = s.webspace_id;
ALTER TABLE changedetection_suburls ALTER COLUMN webspace_host_id SET NOT NULL;
ALTER TABLE changedetection_suburls DROP COLUMN webspace_id;  -- drops UNIQUE(webspace_id, path)
CREATE UNIQUE INDEX uq_cd_suburls_host_path ON changedetection_suburls(webspace_host_id, path);
CREATE INDEX idx_cd_suburls_host ON changedetection_suburls(webspace_host_id);

ALTER TABLE changedetection_notifications
    ADD COLUMN webspace_host_id UUID REFERENCES webspace_hosts(id) ON DELETE CASCADE;
UPDATE changedetection_notifications n
SET webspace_host_id = w.webspace_host_id
FROM webspaces w WHERE w.id = n.webspace_id;
ALTER TABLE changedetection_notifications ALTER COLUMN webspace_host_id SET NOT NULL;
ALTER TABLE changedetection_notifications DROP COLUMN webspace_id;  -- drops idx_cdn_webspace
CREATE INDEX idx_cdn_host ON changedetection_notifications(webspace_host_id, created_at DESC);

-- 7b. Reachability results are per hostname, which is host-owned.
ALTER TABLE reachability_results
    ADD COLUMN webspace_host_id UUID REFERENCES webspace_hosts(id) ON DELETE CASCADE;
UPDATE reachability_results r
SET webspace_host_id = w.webspace_host_id
FROM webspaces w WHERE w.id = r.webspace_id;
DELETE FROM reachability_results WHERE webspace_host_id IS NULL;  -- orphaned; re-created next check
ALTER TABLE reachability_results ALTER COLUMN webspace_host_id SET NOT NULL;
ALTER TABLE reachability_results DROP COLUMN webspace_id;  -- drops idx_reachability_webspace
CREATE INDEX idx_reachability_host ON reachability_results(webspace_host_id);

-- 8. Tighten constraints now that everything is backfilled.
ALTER TABLE webspaces ALTER COLUMN webspace_host_id SET NOT NULL;
ALTER TABLE webspaces DROP CONSTRAINT IF EXISTS webspaces_organization_id_name_key;
ALTER TABLE webspaces DROP COLUMN changedetection_credential_id;

DROP TABLE webspace_domains;
