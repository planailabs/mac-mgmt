CREATE TABLE rollout_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE rollout_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES rollout_groups(id) ON DELETE CASCADE,
    customer_id UUID NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    UNIQUE(group_id, customer_id)
);

CREATE TABLE rollouts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_toml TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'rolling', 'paused', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE rollout_stages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rollout_id UUID NOT NULL REFERENCES rollouts(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES rollout_groups(id),
    stage_order INT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'rolling', 'paused', 'completed', 'failed')),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    UNIQUE(rollout_id, stage_order)
);
