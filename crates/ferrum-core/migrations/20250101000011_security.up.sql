-- OWASP A07: Token revocation
CREATE TABLE IF NOT EXISTS revoked_tokens (
    jti         TEXT PRIMARY KEY,
    revoked_at  TIMESTAMPTZ DEFAULT now(),
    reason      TEXT
);

-- OWASP A09: Security event log
CREATE TABLE IF NOT EXISTS security_events (
    id          TEXT PRIMARY KEY,
    event_type  TEXT NOT NULL,
    severity    TEXT NOT NULL,
    sub         TEXT,
    ip_address  TEXT,
    resource_id TEXT,
    details     JSONB,
    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_security_events_severity ON security_events(severity);
CREATE INDEX IF NOT EXISTS idx_security_events_occurred ON security_events(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_security_events_sub ON security_events(sub);

-- A01: WES run ownership
ALTER TABLE wes_runs ADD COLUMN IF NOT EXISTS owner_sub TEXT;
CREATE INDEX IF NOT EXISTS idx_wes_runs_owner ON wes_runs(owner_sub);

-- A01: Cohort membership (owner + invited members)
CREATE TABLE IF NOT EXISTS cohort_members (
    cohort_id   TEXT NOT NULL REFERENCES cohorts(id) ON DELETE CASCADE,
    sub         TEXT NOT NULL,
    role        TEXT NOT NULL DEFAULT 'member',
    added_at    TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (cohort_id, sub)
);
CREATE INDEX IF NOT EXISTS idx_cohort_members_sub ON cohort_members(sub);
