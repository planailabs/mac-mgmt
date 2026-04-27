-- Merge remote skill center assignments into existing assignment tables.
-- Adds nullable skill_center_id + remote metadata columns; drops the
-- separate cluster_remote_* tables.

-- cluster_skills: add remote-capable columns
ALTER TABLE cluster_skills
  ADD COLUMN IF NOT EXISTS skill_center_id UUID REFERENCES skill_centers(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS remote_id UUID,
  ADD COLUMN IF NOT EXISTS slug TEXT,
  ADD COLUMN IF NOT EXISTS channel TEXT,
  ADD COLUMN IF NOT EXISTS skill_name TEXT NOT NULL DEFAULT '';
ALTER TABLE cluster_skills ALTER COLUMN skill_channel_id DROP NOT NULL;

-- cluster_bundles: add remote-capable columns
ALTER TABLE cluster_bundles
  ADD COLUMN IF NOT EXISTS skill_center_id UUID REFERENCES skill_centers(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS remote_id UUID,
  ADD COLUMN IF NOT EXISTS slug TEXT,
  ADD COLUMN IF NOT EXISTS bundle_name TEXT NOT NULL DEFAULT '';
ALTER TABLE cluster_bundles ALTER COLUMN bundle_id DROP NOT NULL;

-- cluster_mcp_servers: add remote-capable columns
ALTER TABLE cluster_mcp_servers
  ADD COLUMN IF NOT EXISTS skill_center_id UUID REFERENCES skill_centers(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS remote_id UUID,
  ADD COLUMN IF NOT EXISTS slug TEXT,
  ADD COLUMN IF NOT EXISTS mcp_name TEXT NOT NULL DEFAULT '';
ALTER TABLE cluster_mcp_servers ALTER COLUMN mcp_server_id DROP NOT NULL;

-- cluster_mcp_bundles: add remote-capable columns
ALTER TABLE cluster_mcp_bundles
  ADD COLUMN IF NOT EXISTS skill_center_id UUID REFERENCES skill_centers(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS remote_id UUID,
  ADD COLUMN IF NOT EXISTS slug TEXT,
  ADD COLUMN IF NOT EXISTS bundle_name TEXT NOT NULL DEFAULT '';
ALTER TABLE cluster_mcp_bundles ALTER COLUMN bundle_id DROP NOT NULL;

-- Partial unique indexes for remote entries (local uniqueness is already handled)
CREATE UNIQUE INDEX IF NOT EXISTS cluster_skills_remote_unique
  ON cluster_skills (cluster_id, skill_center_id, remote_id)
  WHERE skill_center_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS cluster_bundles_remote_unique
  ON cluster_bundles (cluster_id, skill_center_id, remote_id)
  WHERE skill_center_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS cluster_mcp_servers_remote_unique
  ON cluster_mcp_servers (cluster_id, skill_center_id, remote_id)
  WHERE skill_center_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS cluster_mcp_bundles_remote_unique
  ON cluster_mcp_bundles (cluster_id, skill_center_id, remote_id)
  WHERE skill_center_id IS NOT NULL;

-- Drop the now-redundant remote tables
DROP TABLE IF EXISTS cluster_remote_mcp_bundles;
DROP TABLE IF EXISTS cluster_remote_mcp_servers;
DROP TABLE IF EXISTS cluster_remote_bundles;
DROP TABLE IF EXISTS cluster_remote_skills;
