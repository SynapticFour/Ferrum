-- Metadata Store M2: versioning on ferrum-meta submissions

ALTER TABLE metadata_submissions ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;
ALTER TABLE metadata_submissions ADD COLUMN IF NOT EXISTS updated_time TIMESTAMPTZ;
ALTER TABLE metadata_submissions ADD COLUMN IF NOT EXISTS content_sha256 TEXT NOT NULL DEFAULT '';

UPDATE metadata_submissions SET updated_time = created_time WHERE updated_time IS NULL;

CREATE TABLE IF NOT EXISTS metadata_submission_versions (
    id              TEXT PRIMARY KEY,
    alias           TEXT NOT NULL,
    version         BIGINT NOT NULL,
    profile         TEXT NOT NULL,
    document        TEXT NOT NULL,
    content_sha256  TEXT NOT NULL,
    created_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (alias, version)
);

CREATE INDEX IF NOT EXISTS idx_metadata_submission_versions_alias
    ON metadata_submission_versions (alias);

-- Seed history for existing heads (idempotent via UNIQUE)
INSERT INTO metadata_submission_versions (id, alias, version, profile, document, content_sha256, created_time)
SELECT
    alias || ':v' || version::text,
    alias,
    version,
    profile,
    document,
    content_sha256,
    COALESCE(updated_time, created_time)
FROM metadata_submissions
ON CONFLICT (alias, version) DO NOTHING;
