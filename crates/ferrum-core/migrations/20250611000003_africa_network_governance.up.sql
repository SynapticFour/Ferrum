-- Africa network & governance: transfer checkpoints and residency audit chain.

CREATE TABLE IF NOT EXISTS transfer_checkpoints (
    id              TEXT PRIMARY KEY,
    object_id       TEXT NOT NULL,
    direction       TEXT NOT NULL,
    total_bytes     BIGINT NOT NULL,
    chunk_size      BIGINT NOT NULL,
    completed_bytes BIGINT NOT NULL DEFAULT 0,
    resume_token    TEXT NOT NULL UNIQUE,
    checksum_sha256 TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transfer_checkpoints_object ON transfer_checkpoints(object_id);
CREATE INDEX IF NOT EXISTS idx_transfer_checkpoints_resume ON transfer_checkpoints(resume_token);

CREATE TABLE IF NOT EXISTS residency_audit (
    id                  BIGSERIAL PRIMARY KEY,
    timestamp           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_type          TEXT NOT NULL,
    drs_id              TEXT,
    requester           TEXT,
    destination         TEXT,
    data_left_node      BOOLEAN NOT NULL DEFAULT FALSE,
    bytes_transferred   BIGINT,
    prev_hash           TEXT NOT NULL,
    entry_hash          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_residency_audit_timestamp ON residency_audit(timestamp);
CREATE INDEX IF NOT EXISTS idx_residency_audit_event ON residency_audit(event_type);

CREATE OR REPLACE FUNCTION residency_audit_deny_delete() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'residency_audit is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION residency_audit_deny_update() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'residency_audit is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS residency_audit_no_delete ON residency_audit;
CREATE TRIGGER residency_audit_no_delete
BEFORE DELETE ON residency_audit
FOR EACH ROW EXECUTE FUNCTION residency_audit_deny_delete();

DROP TRIGGER IF EXISTS residency_audit_no_update ON residency_audit;
CREATE TRIGGER residency_audit_no_update
BEFORE UPDATE ON residency_audit
FOR EACH ROW EXECUTE FUNCTION residency_audit_deny_update();
