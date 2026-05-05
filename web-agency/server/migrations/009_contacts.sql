CREATE TABLE domain_contacts (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id      UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    label                TEXT NOT NULL,
    first_name           TEXT NOT NULL,
    last_name            TEXT NOT NULL,
    email                TEXT NOT NULL,
    phone                TEXT NOT NULL,
    address1             TEXT NOT NULL,
    city                 TEXT NOT NULL,
    country              TEXT NOT NULL,
    organization_name    TEXT,
    address2             TEXT,
    state_province       TEXT,
    postal_code          TEXT,
    spaceship_contact_id TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, label)
);

CREATE INDEX idx_contacts_org ON domain_contacts(organization_id);
