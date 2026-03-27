CREATE TABLE skills (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE skill_channels (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id   UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    channel    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(skill_id, channel)
);

CREATE TABLE bundles (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bundle_items (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bundle_id        UUID NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
    skill_channel_id UUID NOT NULL REFERENCES skill_channels(id) ON DELETE CASCADE,
    UNIQUE(bundle_id, skill_channel_id)
);

CREATE TABLE customer_skills (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id      UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    skill_channel_id UUID NOT NULL REFERENCES skill_channels(id) ON DELETE CASCADE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(customer_id, skill_channel_id)
);

CREATE TABLE customer_bundles (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    bundle_id   UUID NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(customer_id, bundle_id)
);
