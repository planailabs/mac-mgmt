-- Cache nameserver sync status on domains.
-- ns_ok: NULL = unknown, true = registrar NS match CF NS, false = mismatch.
ALTER TABLE domains ADD COLUMN ns_ok BOOLEAN;
