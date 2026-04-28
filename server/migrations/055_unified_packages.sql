-- Unified package manager: add nix_packages to skills and manual per-cluster packages.

-- Skill channels can now declare direct nix package dependencies (parallel to mcp_servers.nix_packages).
ALTER TABLE skill_channels ADD COLUMN nix_packages TEXT[] NOT NULL DEFAULT '{}';

-- Manual per-cluster packages: admins can add arbitrary nixpkgs packages to each cluster.
CREATE TABLE cluster_packages (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id UUID NOT NULL REFERENCES clusters(id) ON DELETE CASCADE,
    package    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cluster_id, package)
);
