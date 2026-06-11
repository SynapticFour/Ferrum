CREATE OR REPLACE FUNCTION outbreak_audit_deny_delete() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'outbreak_audit is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION outbreak_audit_deny_update() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'outbreak_audit is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS outbreak_audit_no_delete ON outbreak_audit;
CREATE TRIGGER outbreak_audit_no_delete
BEFORE DELETE ON outbreak_audit
FOR EACH ROW EXECUTE FUNCTION outbreak_audit_deny_delete();

DROP TRIGGER IF EXISTS outbreak_audit_no_update ON outbreak_audit;
CREATE TRIGGER outbreak_audit_no_update
BEFORE UPDATE ON outbreak_audit
FOR EACH ROW EXECUTE FUNCTION outbreak_audit_deny_update();
