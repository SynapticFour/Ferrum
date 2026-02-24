DROP INDEX IF EXISTS idx_checkpoints_hash;
DROP INDEX IF EXISTS idx_checkpoints_run;
DROP TABLE IF EXISTS wes_cache_entries;
DROP TABLE IF EXISTS wes_checkpoints;
ALTER TABLE wes_runs DROP COLUMN IF EXISTS checkpoint_enabled;
ALTER TABLE wes_runs DROP COLUMN IF EXISTS resumed_from_run_id;
