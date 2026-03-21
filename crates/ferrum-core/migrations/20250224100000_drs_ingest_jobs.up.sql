-- Lab Kit / machine ingest: job records for register & upload flows (idempotency, polling).
CREATE TABLE drs_ingest_jobs (
    id                  TEXT PRIMARY KEY,
    client_request_id   TEXT UNIQUE,
    job_type            TEXT NOT NULL,
    status              TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    result_json         JSONB,
    error_json          JSONB
);

CREATE INDEX idx_drs_ingest_jobs_status ON drs_ingest_jobs (status);
CREATE INDEX idx_drs_ingest_jobs_created ON drs_ingest_jobs (created_at);
