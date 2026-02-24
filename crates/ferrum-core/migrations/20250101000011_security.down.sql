DROP INDEX IF EXISTS idx_cohort_members_sub;
DROP TABLE IF EXISTS cohort_members;
DROP INDEX IF EXISTS idx_wes_runs_owner;
ALTER TABLE wes_runs DROP COLUMN IF EXISTS owner_sub;
DROP INDEX IF EXISTS idx_security_events_sub;
DROP INDEX IF EXISTS idx_security_events_occurred;
DROP INDEX IF EXISTS idx_security_events_severity;
DROP TABLE IF EXISTS security_events;
DROP TABLE IF EXISTS revoked_tokens;
