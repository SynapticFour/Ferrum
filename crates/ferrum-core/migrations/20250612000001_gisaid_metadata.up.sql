-- GISAID submission metadata captured at ingest (optional JSONB on DRS objects).

ALTER TABLE drs_objects ADD COLUMN IF NOT EXISTS gisaid_metadata JSONB;
