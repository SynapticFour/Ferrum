-- Field sync queue for Edge → hub upload (ADR-019 / Phase 4).

CREATE TABLE IF NOT EXISTS sync_queue (
    id              TEXT PRIMARY KEY,
    object_id       TEXT NOT NULL,
    target_url      TEXT NOT NULL,
    state           TEXT NOT NULL DEFAULT 'pending',
    bytes_total     BIGINT NOT NULL DEFAULT 0,
    bytes_sent      BIGINT NOT NULL DEFAULT 0,
    resume_token    TEXT,
    crypt4gh        BOOLEAN NOT NULL DEFAULT FALSE,
    metadata_ref    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_attempt_at TIMESTAMPTZ,
    error_message   TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_state ON sync_queue(state);
CREATE INDEX IF NOT EXISTS idx_sync_queue_object ON sync_queue(object_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_target ON sync_queue(target_url);
