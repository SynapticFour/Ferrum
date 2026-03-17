-- GA4GH TRS 2.0.1: tools, tool_versions, descriptor/test/container files.
-- Replace minimal trs_tools from 00001_initial with full schema.

DROP TABLE IF EXISTS trs_files CASCADE;
DROP TABLE IF EXISTS trs_tool_versions CASCADE;
DROP TABLE IF EXISTS trs_tools CASCADE;

CREATE TABLE trs_tools (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    description     TEXT,
    organization     TEXT,
    toolclass       TEXT,
    meta_version    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE trs_tool_versions (
    id              TEXT PRIMARY KEY,
    tool_id         TEXT NOT NULL REFERENCES trs_tools(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tool_id, name)
);

CREATE INDEX idx_trs_tool_versions_tool_id ON trs_tool_versions(tool_id);

CREATE TABLE trs_files (
    id              BIGSERIAL PRIMARY KEY,
    tool_id         TEXT NOT NULL REFERENCES trs_tools(id) ON DELETE CASCADE,
    version_id      TEXT NOT NULL REFERENCES trs_tool_versions(id) ON DELETE CASCADE,
    file_type       TEXT NOT NULL,  -- 'DESCRIPTOR' | 'TEST_FILE' | 'CONTAINERFILE'
    descriptor_type TEXT,           -- 'CWL' | 'WDL' | 'NFL' | 'SMK' | 'PLAIN'
    content         TEXT,
    url             TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trs_files_tool_version ON trs_files(tool_id, version_id);
