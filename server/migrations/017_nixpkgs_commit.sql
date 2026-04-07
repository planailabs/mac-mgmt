-- Per-customer pinned nixpkgs commit, optionally driven via rollouts.
-- NULL on customers => use the rolling default source.
-- NULL on rollouts  => the rollout doesn't change nixpkgs.

ALTER TABLE customers ADD COLUMN nixpkgs_commit TEXT;
ALTER TABLE rollouts  ADD COLUMN nixpkgs_commit TEXT;
