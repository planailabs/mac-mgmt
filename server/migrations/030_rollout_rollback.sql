-- Rollback support: capture what the cohort was running *before* the
-- rollout started so we have somewhere safe to retreat to when an
-- assessment gate (or a human) decides the new version is bad.
--
-- The baseline is populated when the first stage flips from pending ->
-- rolling. If either field is NULL at rollback time the update evaluator
-- falls back to the cluster's pinned_version / latest daemon_versions
-- row, matching the no-rollout code path.
ALTER TABLE rollouts ADD COLUMN IF NOT EXISTS baseline_version TEXT;
ALTER TABLE rollouts ADD COLUMN IF NOT EXISTS baseline_nixpkgs_commit TEXT;

-- Extend the rollout + stage status check to include 'rolled_back'. The
-- only way to drop a CHECK constraint portably is drop + recreate; the
-- name of the original constraint is stable because Postgres derives it
-- from the column name.
ALTER TABLE rollouts DROP CONSTRAINT IF EXISTS rollouts_status_check;
ALTER TABLE rollouts ADD CONSTRAINT rollouts_status_check
    CHECK (status IN ('pending', 'rolling', 'paused', 'completed', 'failed', 'rolled_back'));

ALTER TABLE rollout_stages DROP CONSTRAINT IF EXISTS rollout_stages_status_check;
ALTER TABLE rollout_stages ADD CONSTRAINT rollout_stages_status_check
    CHECK (status IN ('pending', 'rolling', 'paused', 'completed', 'failed', 'rolled_back'));
