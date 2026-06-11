DROP TABLE IF EXISTS outbreak_download_approvals;
DROP TABLE IF EXISTS outbreak_audit;
DROP TABLE IF EXISTS outbreak_activations;
DROP TABLE IF EXISTS pathogen_annotations;
ALTER TABLE drs_objects DROP COLUMN IF EXISTS ont_metrics;
