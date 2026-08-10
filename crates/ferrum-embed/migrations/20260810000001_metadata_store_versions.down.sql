DROP INDEX IF EXISTS idx_metadata_submission_versions_alias;
DROP TABLE IF EXISTS metadata_submission_versions;
-- SQLite cannot DROP COLUMN portably across all Edge versions; leave columns if present.
