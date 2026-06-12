-- Row-level security for sensitive Africa / governance tables (PostgreSQL).
-- Ferrum's application pool uses a single DB role; policies allow the service role
-- full access while documenting tenant isolation for deployments that set
-- `app.current_requester` per connection.

CREATE OR REPLACE FUNCTION ferrum_current_requester() RETURNS text AS $$
  SELECT NULLIF(current_setting('app.current_requester', true), '');
$$ LANGUAGE sql STABLE;

ALTER TABLE residency_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE outbreak_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE pathogen_annotations ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS residency_audit_service ON residency_audit;
CREATE POLICY residency_audit_service ON residency_audit
  FOR ALL
  USING (ferrum_current_requester() IS NULL OR requester IS NULL OR requester = ferrum_current_requester())
  WITH CHECK (true);

DROP POLICY IF EXISTS outbreak_audit_service ON outbreak_audit;
CREATE POLICY outbreak_audit_service ON outbreak_audit
  FOR ALL
  USING (true)
  WITH CHECK (true);

DROP POLICY IF EXISTS pathogen_annotations_service ON pathogen_annotations;
CREATE POLICY pathogen_annotations_service ON pathogen_annotations
  FOR ALL
  USING (true)
  WITH CHECK (true);
