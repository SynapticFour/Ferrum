DROP INDEX IF EXISTS idx_metadata_submission_versions_alias;
DROP TABLE IF EXISTS metadata_submission_versions;
ALTER TABLE metadata_submissions DROP COLUMN IF EXISTS content_sha256;
ALTER TABLE metadata_submissions DROP COLUMN IF EXISTS updated_time;
ALTER TABLE metadata_submissions DROP COLUMN IF EXISTS version;
