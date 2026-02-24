DROP INDEX IF EXISTS idx_drs_objects_dataset_id;
ALTER TABLE drs_objects DROP COLUMN IF EXISTS dataset_id;
