-- Metadata Store M2: versioning on ferrum-meta submissions (SQLite / Edge)

ALTER TABLE metadata_submissions ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_submissions ADD COLUMN updated_time TEXT;
ALTER TABLE metadata_submissions ADD COLUMN content_sha256 TEXT NOT NULL DEFAULT '';

UPDATE metadata_submissions SET updated_time = created_time WHERE updated_time IS NULL;

CREATE TABLE IF NOT EXISTS metadata_submission_versions (
    id              TEXT PRIMARY KEY,
    alias           TEXT NOT NULL,
    version         INTEGER NOT NULL,
    profile         TEXT NOT NULL,
    document        TEXT NOT NULL,
    content_sha256  TEXT NOT NULL,
    created_time    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (alias, version)
);

CREATE INDEX IF NOT EXISTS idx_metadata_submission_versions_alias
    ON metadata_submission_versions (alias);

INSERT OR IGNORE INTO metadata_submission_versions (id, alias, version, profile, document, content_sha256, created_time)
SELECT
    alias || ':v' || CAST(version AS TEXT),
    alias,
    version,
    profile,
    document,
    content_sha256,
    COALESCE(updated_time, created_time)
FROM metadata_submissions;
