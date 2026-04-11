-- Rename "customers" to "clusters" throughout the schema.
-- PostgreSQL ALTER TABLE RENAME is a metadata-only operation (instant).

ALTER TABLE customers RENAME TO clusters;

ALTER TABLE customer_configs RENAME TO cluster_configs;
ALTER TABLE cluster_configs RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE customer_skills RENAME TO cluster_skills;
ALTER TABLE cluster_skills RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE customer_bundles RENAME TO cluster_bundles;
ALTER TABLE cluster_bundles RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE customer_mcp_servers RENAME TO cluster_mcp_servers;
ALTER TABLE cluster_mcp_servers RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE customer_mcp_bundles RENAME TO cluster_mcp_bundles;
ALTER TABLE cluster_mcp_bundles RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE customer_ssh_keys RENAME TO cluster_ssh_keys;
ALTER TABLE cluster_ssh_keys RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE daemon_heartbeats RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE tokens RENAME COLUMN customer_id TO cluster_id;

ALTER TABLE rollout_group_members RENAME COLUMN customer_id TO cluster_id;

-- Rename the "All Customers" sentinel rollout group
UPDATE rollout_groups SET name = 'All Clusters'
WHERE id = '00000000-0000-0000-0000-000000000000';
