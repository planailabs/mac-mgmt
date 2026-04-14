-- Cascade deletion for per-instance data.
--
-- Before this migration `assessments` and `assessment_probes` carried
-- (cluster_id, instance_id) as plain columns with no FK. Any path that
-- deleted a row from `daemon_heartbeats` (the 30d cleanup cron, the
-- fleet-dashboard delete button, a cluster cascade, raw SQL) left
-- matching assessment + probe rows orphaned. Those rows never get
-- touched by the update evaluator or the health gate, but they do keep
-- showing up in per-instance queries for an instance that no longer
-- exists.
--
-- Fix by making daemon_heartbeats the parent row:
--   1. Delete any pre-existing orphans so the FK add doesn't fail.
--   2. Add ON DELETE CASCADE FKs on both child tables, keyed on the
--      same (cluster_id, instance_id) pair that's already UNIQUE on
--      daemon_heartbeats (migration 009 + rename in 021).
--
-- Race note: a brand-new daemon that POSTs /api/assessment before its
-- first /api/heartbeat will now hit an FK violation and get 500. The
-- daemon retries on its 6h cadence and the second attempt succeeds
-- once the heartbeat has landed. Acceptable trade-off — the alternative
-- (no FK) was the bug we're fixing.

DELETE FROM assessments a
 WHERE NOT EXISTS (
     SELECT 1 FROM daemon_heartbeats h
      WHERE h.cluster_id = a.cluster_id
        AND h.instance_id = a.instance_id
 );

DELETE FROM assessment_probes p
 WHERE NOT EXISTS (
     SELECT 1 FROM daemon_heartbeats h
      WHERE h.cluster_id = p.cluster_id
        AND h.instance_id = p.instance_id
 );

ALTER TABLE assessments
    ADD CONSTRAINT assessments_daemon_heartbeat_fk
    FOREIGN KEY (cluster_id, instance_id)
    REFERENCES daemon_heartbeats(cluster_id, instance_id)
    ON DELETE CASCADE;

ALTER TABLE assessment_probes
    ADD CONSTRAINT assessment_probes_daemon_heartbeat_fk
    FOREIGN KEY (cluster_id, instance_id)
    REFERENCES daemon_heartbeats(cluster_id, instance_id)
    ON DELETE CASCADE;
