-- Sentinel row for "All Customers": nil UUID in rollout_stages.group_id
-- means "every customer" and queries resolve it via subquery instead of
-- joining through rollout_group_members.
INSERT INTO rollout_groups (id, name, description)
VALUES ('00000000-0000-0000-0000-000000000000', 'All Customers', 'Sentinel – represents every customer')
ON CONFLICT (id) DO NOTHING;
