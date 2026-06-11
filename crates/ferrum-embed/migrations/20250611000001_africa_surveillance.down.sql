DROP TABLE IF EXISTS outbreak_download_approvals;
DROP TABLE IF EXISTS outbreak_audit;
DROP TABLE IF EXISTS outbreak_activations;
DROP TABLE IF EXISTS pathogen_annotations;
-- SQLite cannot DROP COLUMN in older versions; leave ont_metrics if present.
