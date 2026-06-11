DROP TRIGGER IF EXISTS residency_audit_no_update ON residency_audit;
DROP TRIGGER IF EXISTS residency_audit_no_delete ON residency_audit;
DROP FUNCTION IF EXISTS residency_audit_deny_update();
DROP FUNCTION IF EXISTS residency_audit_deny_delete();
DROP TABLE IF EXISTS residency_audit;
DROP TABLE IF EXISTS transfer_checkpoints;
