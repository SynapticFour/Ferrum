CREATE TABLE IF NOT EXISTS sync_queue (
    id              TEXT PRIMARY KEY,
    object_id       TEXT NOT NULL,
    target_url      TEXT NOT NULL,
    state           TEXT NOT NULL DEFAULT 'pending',
    bytes_total     INTEGER NOT NULL DEFAULT 0,
    bytes_sent      INTEGER NOT NULL DEFAULT 0,
    resume_token    TEXT,
    crypt4gh        INTEGER NOT NULL DEFAULT 0,
    metadata_ref    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_attempt_at TEXT,
    error_message   TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_state ON sync_queue(state);
CREATE INDEX IF NOT EXISTS idx_sync_queue_object ON sync_queue(object_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_target ON sync_queue(target_url);
