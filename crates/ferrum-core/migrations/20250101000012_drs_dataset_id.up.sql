-- A01: DRS dataset-level access control (ControlledAccessGrants visa).
ALTER TABLE drs_objects ADD COLUMN IF NOT EXISTS dataset_id TEXT;
CREATE INDEX IF NOT EXISTS idx_drs_objects_dataset_id ON drs_objects(dataset_id) WHERE dataset_id IS NOT NULL;
