CREATE TABLE skill_mcp_dependencies (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_channel_id UUID NOT NULL REFERENCES skill_channels(id) ON DELETE CASCADE,
    mcp_server_id    UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    UNIQUE(skill_channel_id, mcp_server_id)
);
