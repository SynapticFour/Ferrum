CREATE TABLE IF NOT EXISTS transfer_checkpoints (
    id              TEXT PRIMARY KEY,
    object_id       TEXT NOT NULL,
    direction       TEXT NOT NULL,
    total_bytes     INTEGER NOT NULL,
    chunk_size      INTEGER NOT NULL,
    completed_bytes INTEGER NOT NULL DEFAULT 0,
    resume_token    TEXT NOT NULL UNIQUE,
    checksum_sha256 TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_transfer_checkpoints_object ON transfer_checkpoints(object_id);
CREATE INDEX IF NOT EXISTS idx_transfer_checkpoints_resume ON transfer_checkpoints(resume_token);

CREATE TABLE IF NOT EXISTS residency_audit (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp           TEXT NOT NULL DEFAULT (datetime('now')),
    event_type          TEXT NOT NULL,
    drs_id              TEXT,
    requester           TEXT,
    destination         TEXT,
    data_left_node      INTEGER NOT NULL DEFAULT 0,
    bytes_transferred   INTEGER,
    prev_hash           TEXT NOT NULL,
    entry_hash          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_residency_audit_timestamp ON residency_audit(timestamp);
CREATE INDEX IF NOT EXISTS idx_residency_audit_event ON residency_audit(event_type);

CREATE TRIGGER IF NOT EXISTS residency_audit_no_delete
BEFORE DELETE ON residency_audit
BEGIN
  SELECT RAISE(ABORT, 'residency_audit is append-only');
END;

CREATE TRIGGER IF NOT EXISTS residency_audit_no_update
BEFORE UPDATE ON residency_audit
BEGIN
  SELECT RAISE(ABORT, 'residency_audit is append-only');
END;
