ALTER TABLE drs_objects ADD COLUMN metadata_ref TEXT;

CREATE TABLE IF NOT EXISTS metadata_submissions (
    id          TEXT PRIMARY KEY,
    alias       TEXT NOT NULL UNIQUE,
    profile     TEXT NOT NULL,
    document    TEXT NOT NULL,
    created_time TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_metadata_submissions_alias ON metadata_submissions(alias);
CREATE INDEX IF NOT EXISTS idx_drs_objects_metadata_ref ON drs_objects(metadata_ref);
