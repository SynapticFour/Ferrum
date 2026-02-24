-- DRS 1.4 full schema (replaces minimal drs tables from initial migration)
DROP TABLE IF EXISTS drs_access_log;
DROP TABLE IF EXISTS storage_references;
DROP TABLE IF EXISTS drs_object_metadata;
DROP TABLE IF EXISTS drs_bundle_contents;
DROP TABLE IF EXISTS drs_access_methods;
DROP TABLE IF EXISTS drs_checksums;
DROP TABLE IF EXISTS drs_objects;

CREATE TABLE drs_objects (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    description     TEXT,
    created_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_time    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version         TEXT,
    mime_type       TEXT,
    size            BIGINT NOT NULL DEFAULT 0,
    is_bundle       BOOLEAN NOT NULL DEFAULT FALSE,
    aliases         JSONB DEFAULT '[]'::jsonb
);

CREATE INDEX idx_drs_objects_aliases ON drs_objects USING GIN (aliases);
CREATE INDEX idx_drs_objects_mime_type ON drs_objects(mime_type);
CREATE INDEX idx_drs_objects_size ON drs_objects(size);
CREATE INDEX idx_drs_objects_created_time ON drs_objects(created_time);

CREATE TABLE drs_checksums (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    checksum    TEXT NOT NULL,
    PRIMARY KEY (object_id, type)
);

CREATE TABLE drs_access_methods (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    access_id   TEXT,
    access_url  JSONB,
    region      TEXT,
    headers     JSONB DEFAULT '[]'::jsonb,
    PRIMARY KEY (object_id, type)
);

CREATE TABLE drs_bundle_contents (
    bundle_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    drs_uri     TEXT,
    PRIMARY KEY (bundle_id, object_id)
);

CREATE TABLE drs_object_metadata (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT,
    PRIMARY KEY (object_id, key)
);

CREATE TABLE storage_references (
    object_id       TEXT NOT NULL PRIMARY KEY REFERENCES drs_objects(id) ON DELETE CASCADE,
    storage_backend TEXT NOT NULL,
    storage_key     TEXT NOT NULL,
    is_encrypted    BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE drs_access_log (
    id          BIGSERIAL PRIMARY KEY,
    object_id   TEXT NOT NULL,
    access_id   TEXT,
    method      TEXT,
    status      INT,
    client_ip   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_drs_access_log_object_id ON drs_access_log(object_id);
CREATE INDEX idx_drs_access_log_created_at ON drs_access_log(created_at);
