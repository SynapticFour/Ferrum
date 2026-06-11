CREATE TRIGGER IF NOT EXISTS outbreak_audit_no_delete
BEFORE DELETE ON outbreak_audit
BEGIN
  SELECT RAISE(ABORT, 'outbreak_audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS outbreak_audit_no_update
BEFORE UPDATE ON outbreak_audit
BEGIN
  SELECT RAISE(ABORT, 'outbreak_audit is append-only');
END;
