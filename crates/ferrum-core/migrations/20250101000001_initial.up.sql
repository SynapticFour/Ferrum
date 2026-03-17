-- Ferrum initial schema (PostgreSQL)
-- GA4GH DRS objects
CREATE TABLE IF NOT EXISTS drs_objects (
    id          TEXT PRIMARY KEY,
    name        TEXT,
    size        BIGINT,
    created_time TIMESTAMPTZ DEFAULT NOW(),
    updated_time TIMESTAMPTZ DEFAULT NOW(),
    description TEXT,
    storage_key TEXT NOT NULL
);

-- Index only if storage_key exists (00002 replaces drs_objects and drops this column)
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'drs_objects' AND column_name = 'storage_key'
  ) THEN
    CREATE INDEX IF NOT EXISTS idx_drs_objects_storage_key ON drs_objects(storage_key);
  END IF;
END $$;

-- DRS checksums (one row per checksum per object)
CREATE TABLE IF NOT EXISTS drs_checksums (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    checksum    TEXT NOT NULL,
    PRIMARY KEY (object_id, type)
);

-- DRS access methods
CREATE TABLE IF NOT EXISTS drs_access_methods (
    object_id   TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,
    access_id   TEXT,
    access_url  JSONB,
    region      TEXT,
    PRIMARY KEY (object_id, type)
);

-- Tool Registry (TRS) tools
CREATE TABLE IF NOT EXISTS trs_tools (
    id          TEXT PRIMARY KEY,
    name        TEXT,
    version     TEXT,
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Workflow runs (WES)
CREATE TABLE IF NOT EXISTS wes_runs (
    id          TEXT PRIMARY KEY,
    state       TEXT NOT NULL DEFAULT 'UNKNOWN',
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Task runs (TES)
CREATE TABLE IF NOT EXISTS tes_tasks (
    id          TEXT PRIMARY KEY,
    state       TEXT NOT NULL DEFAULT 'UNKNOWN',
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);
