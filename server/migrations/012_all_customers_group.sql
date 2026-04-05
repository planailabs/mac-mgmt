-- Allow rollout stages to target all customers (group_id = NULL).
ALTER TABLE rollout_stages ALTER COLUMN group_id DROP NOT NULL;
