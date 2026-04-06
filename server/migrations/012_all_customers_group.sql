-- Remove NULL group_id stages (the "all customers" special case is gone;
-- use a group containing all customers instead).
DELETE FROM rollout_stages WHERE group_id IS NULL;
ALTER TABLE rollout_stages ALTER COLUMN group_id SET NOT NULL;
