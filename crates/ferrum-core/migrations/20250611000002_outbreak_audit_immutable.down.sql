DROP TRIGGER IF EXISTS outbreak_audit_no_update ON outbreak_audit;
DROP TRIGGER IF EXISTS outbreak_audit_no_delete ON outbreak_audit;
DROP FUNCTION IF EXISTS outbreak_audit_deny_update();
DROP FUNCTION IF EXISTS outbreak_audit_deny_delete();
