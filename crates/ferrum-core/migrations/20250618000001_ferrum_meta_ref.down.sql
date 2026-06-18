DROP INDEX IF EXISTS idx_drs_objects_metadata_ref;
DROP INDEX IF EXISTS idx_metadata_submissions_alias;
DROP TABLE IF EXISTS metadata_submissions;
ALTER TABLE drs_objects DROP COLUMN IF EXISTS metadata_ref;
