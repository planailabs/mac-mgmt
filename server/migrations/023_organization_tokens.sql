-- Allow tokens to be scoped to an organization instead of a single cluster.
-- organization_id set + cluster_id NULL = org-scoped setting token
-- cluster_id set + organization_id NULL = single-cluster token (existing)
-- both NULL = admin token (existing)
ALTER TABLE tokens ADD COLUMN organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE;
