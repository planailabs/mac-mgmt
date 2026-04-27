-- Registered remote skill centers (management servers pull catalogs from these)
CREATE TABLE IF NOT EXISTS skill_centers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    url TEXT NOT NULL UNIQUE,
    federation_token TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Remote skill assignments (skills from external skill centers assigned to local clusters)
CREATE TABLE IF NOT EXISTS cluster_remote_skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    skill_center_id UUID NOT NULL REFERENCES skill_centers(id) ON DELETE CASCADE,
    remote_skill_channel_id UUID NOT NULL,
    slug TEXT NOT NULL,
    channel TEXT NOT NULL,
    skill_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cluster_id, skill_center_id, remote_skill_channel_id)
);

-- Remote bundle assignments
CREATE TABLE IF NOT EXISTS cluster_remote_bundles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    skill_center_id UUID NOT NULL REFERENCES skill_centers(id) ON DELETE CASCADE,
    remote_bundle_id UUID NOT NULL,
    slug TEXT NOT NULL,
    bundle_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cluster_id, skill_center_id, remote_bundle_id)
);

-- Remote MCP server assignments
CREATE TABLE IF NOT EXISTS cluster_remote_mcp_servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    skill_center_id UUID NOT NULL REFERENCES skill_centers(id) ON DELETE CASCADE,
    remote_mcp_server_id UUID NOT NULL,
    slug TEXT NOT NULL,
    mcp_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cluster_id, skill_center_id, remote_mcp_server_id)
);

-- Remote MCP bundle assignments
CREATE TABLE IF NOT EXISTS cluster_remote_mcp_bundles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    skill_center_id UUID NOT NULL REFERENCES skill_centers(id) ON DELETE CASCADE,
    remote_bundle_id UUID NOT NULL,
    slug TEXT NOT NULL,
    bundle_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cluster_id, skill_center_id, remote_bundle_id)
);
