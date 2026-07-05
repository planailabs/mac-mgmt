-- Gradual release within a stage: over ramp_minutes from stage start, a
-- growing deterministic fraction of the group's clusters becomes eligible
-- for the rollout target (hash-bucketed per rollout+cluster). NULL keeps
-- the instant all-at-once behavior.
ALTER TABLE rollout_stages ADD COLUMN ramp_minutes INT
    CHECK (ramp_minutes IS NULL OR ramp_minutes > 0);
