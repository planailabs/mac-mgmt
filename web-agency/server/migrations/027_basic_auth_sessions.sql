-- Cookie-based basic-auth gate (replaces HTTP Basic challenge).
--
-- A browser identity (`sso`) proves credentials for one or more lists. The
-- identity is referenced by cookies on multiple domains: one on the agency
-- domain (for silent SSO across sites sharing a list) plus one per webspace
-- domain (the actual gate cookie the proxy verifies). List authorizations live
-- on the identity, so logging into one site silently covers every other site
-- sharing that list.

-- The browser identity.
CREATE TABLE basic_auth_sso (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL
);

-- Which lists this identity proved, and as whom.
CREATE TABLE basic_auth_sso_lists (
    sso_id    UUID NOT NULL REFERENCES basic_auth_sso(id) ON DELETE CASCADE,
    list_id   UUID NOT NULL REFERENCES basic_auth_lists(id) ON DELETE CASCADE,
    username  TEXT NOT NULL,
    PRIMARY KEY (sso_id, list_id)
);

-- One row per cookie token (agency cookie + each webspace-domain cookie).
-- token_hash is sha256(opaque token); the raw token is never stored.
CREATE TABLE basic_auth_sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sso_id      UUID NOT NULL REFERENCES basic_auth_sso(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_basic_auth_sessions_sso ON basic_auth_sessions(sso_id);
