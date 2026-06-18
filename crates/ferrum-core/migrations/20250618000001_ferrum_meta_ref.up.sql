-- ferrum-meta submission binding (Phase 2 / T3)

ALTER TABLE drs_objects ADD COLUMN IF NOT EXISTS metadata_ref TEXT;

CREATE TABLE IF NOT EXISTS metadata_submissions (
    id          TEXT PRIMARY KEY,
    alias       TEXT NOT NULL UNIQUE,
    profile     TEXT NOT NULL,
    document    TEXT NOT NULL,
    created_time TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_metadata_submissions_alias ON metadata_submissions(alias);
CREATE INDEX IF NOT EXISTS idx_drs_objects_metadata_ref ON drs_objects(metadata_ref) WHERE metadata_ref IS NOT NULL;
