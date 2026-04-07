-- Allow rollouts that only change nixpkgs_commit without bumping the daemon
-- version. At least one of (target_version, nixpkgs_commit) must be set;
-- enforced by a CHECK constraint.

ALTER TABLE rollouts ALTER COLUMN target_version DROP NOT NULL;
ALTER TABLE rollouts ADD CONSTRAINT rollouts_target_or_commit_chk
    CHECK (target_version IS NOT NULL OR nixpkgs_commit IS NOT NULL);
