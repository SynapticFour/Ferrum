-- GA4GH TES 1.1: task execution tasks.
-- state: UNKNOWN, QUEUED, INITIALIZING, RUNNING, PAUSED, COMPLETE, EXECUTOR_ERROR, SYSTEM_ERROR, CANCELED, CANCELING

CREATE TABLE tes_tasks (
    id              TEXT PRIMARY KEY,
    state           TEXT NOT NULL DEFAULT 'QUEUED',
    name            TEXT,
    description     TEXT,
    inputs          JSONB DEFAULT '[]',
    outputs         JSONB DEFAULT '[]',
    executors       JSONB NOT NULL,
    resources       JSONB,
    volumes         JSONB,
    tags            JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at      TIMESTAMPTZ,
    ended_at        TIMESTAMPTZ,
    external_id     TEXT,
    backend         TEXT,
    logs            JSONB
);

CREATE INDEX idx_tes_tasks_state ON tes_tasks(state);
CREATE INDEX idx_tes_tasks_created_at ON tes_tasks(created_at DESC);
