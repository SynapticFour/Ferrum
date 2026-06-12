DROP POLICY IF EXISTS pathogen_annotations_service ON pathogen_annotations;
DROP POLICY IF EXISTS outbreak_audit_service ON outbreak_audit;
DROP POLICY IF EXISTS residency_audit_service ON residency_audit;

ALTER TABLE pathogen_annotations DISABLE ROW LEVEL SECURITY;
ALTER TABLE outbreak_audit DISABLE ROW LEVEL SECURITY;
ALTER TABLE residency_audit DISABLE ROW LEVEL SECURITY;

DROP FUNCTION IF EXISTS ferrum_current_requester();
