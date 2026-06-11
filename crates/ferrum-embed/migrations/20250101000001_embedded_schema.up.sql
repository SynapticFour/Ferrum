-- Embedded / SQLite schema (portable subset of ferrum-core PostgreSQL migrations).

CREATE TABLE IF NOT EXISTS drs_objects (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    description     TEXT,
    created_time    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_time    TEXT NOT NULL DEFAULT (datetime('now')),
    version         TEXT,
    mime_type       TEXT,
    size            INTEGER NOT NULL DEFAULT 0,
    is_bundle       INTEGER NOT NULL DEFAULT 0,
    aliases         TEXT DEFAULT '[]',
    dataset_id      TEXT,
    workspace_id    TEXT
);

CREATE INDEX IF NOT EXISTS idx_drs_objects_mime_type ON drs_objects(mime_type);
CREATE INDEX IF NOT EXISTS idx_drs_objects_size ON drs_objects(size);
CREATE INDEX IF NOT EXISTS idx_drs_objects_created_time ON drs_objects(created_time);
CREATE INDEX IF NOT EXISTS idx_drs_objects_dataset_id ON drs_objects(dataset_id);

CREATE TABLE IF NOT EXISTS drs_checksums (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    checksum    TEXT NOT NULL,
    PRIMARY KEY (object_id, type)
);

CREATE TABLE IF NOT EXISTS drs_access_methods (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    access_id   TEXT,
    access_url  TEXT,
    region      TEXT,
    headers     TEXT DEFAULT '[]',
    PRIMARY KEY (object_id, type)
);

CREATE TABLE IF NOT EXISTS drs_bundle_contents (
    bundle_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    drs_uri     TEXT,
    PRIMARY KEY (bundle_id, object_id)
);

CREATE TABLE IF NOT EXISTS drs_object_metadata (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT,
    PRIMARY KEY (object_id, key)
);

CREATE TABLE IF NOT EXISTS storage_references (
    object_id       TEXT NOT NULL PRIMARY KEY REFERENCES drs_objects(id) ON DELETE CASCADE,
    storage_backend TEXT NOT NULL,
    storage_key     TEXT NOT NULL,
    is_encrypted    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS drs_access_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    object_id   TEXT NOT NULL,
    access_id   TEXT,
    method      TEXT,
    status      INTEGER,
    client_ip   TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_drs_access_log_object_id ON drs_access_log(object_id);

CREATE TABLE IF NOT EXISTS drs_ingest_jobs (
    id                  TEXT PRIMARY KEY,
    client_request_id   TEXT UNIQUE,
    job_type            TEXT NOT NULL,
    status              TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    result_json         TEXT,
    error_json          TEXT
);

CREATE INDEX IF NOT EXISTS idx_drs_ingest_jobs_status ON drs_ingest_jobs(status);

CREATE TABLE IF NOT EXISTS beacon_datasets (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    description     TEXT,
    assembly_id     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS beacon_variants (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    dataset_id      TEXT NOT NULL REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    chromosome      TEXT NOT NULL,
    start           INTEGER NOT NULL,
    "end"           INTEGER NOT NULL,
    reference       TEXT,
    alternate       TEXT,
    variant_type    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_beacon_variants_dataset ON beacon_variants(dataset_id);
CREATE INDEX IF NOT EXISTS idx_beacon_variants_chr_start ON beacon_variants(chromosome, start, "end");
