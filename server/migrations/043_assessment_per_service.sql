-- Per-service inventory and security findings from assessments.
ALTER TABLE assessments ADD COLUMN service_inventories JSONB;
ALTER TABLE assessments ADD COLUMN service_security JSONB;
