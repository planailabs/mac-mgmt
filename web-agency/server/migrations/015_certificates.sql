CREATE TABLE certificates (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain                 TEXT NOT NULL UNIQUE,
    encrypted_chain        BYTEA NOT NULL,
    encrypted_key          BYTEA NOT NULL,
    encrypted_acme_account BYTEA,
    issuer                 TEXT NOT NULL DEFAULT 'self-signed',
    not_before             TIMESTAMPTZ NOT NULL,
    not_after              TIMESTAMPTZ NOT NULL,
    acme_status            TEXT DEFAULT 'none'
                           CHECK (acme_status IN ('none', 'pending', 'valid', 'failed')),
    last_error             TEXT,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_certificates_expiry ON certificates(not_after);
