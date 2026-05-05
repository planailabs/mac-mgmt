-- Track tarball uploads and wrangler deploy status
CREATE TABLE deployments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webspace_id     UUID NOT NULL REFERENCES webspaces(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'uploading', 'deploying', 'success', 'failed')),
    error_message   TEXT,
    tarball_size    BIGINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_deployments_webspace ON deployments(webspace_id);

-- Add 'deploy' as a valid token kind by allowing it in the tokens table.
-- The existing CHECK constraint (if any) is flexible since kind is TEXT.
-- Deploy tokens carry a webspace_id scope in their scopes JSONB field.
