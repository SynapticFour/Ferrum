-- WES 1.1 schema: workflow runs, run log, task logs.
-- state: UNKNOWN, QUEUED, INITIALIZING, RUNNING, PAUSED, COMPLETE, EXECUTOR_ERROR, SYSTEM_ERROR, CANCELED, CANCELING, PREEMPTED
-- Replace minimal wes_runs from 00001_initial with full schema.

DROP TABLE IF EXISTS wes_task_logs CASCADE;
DROP TABLE IF EXISTS wes_run_log CASCADE;
DROP TABLE IF EXISTS wes_runs CASCADE;

CREATE TABLE wes_runs (
    run_id                  TEXT PRIMARY KEY,
    workflow_url            TEXT NOT NULL,
    workflow_type           TEXT NOT NULL,
    workflow_type_version   TEXT NOT NULL,
    workflow_params         JSONB DEFAULT '{}',
    workflow_engine_params  JSONB DEFAULT '{}',
    tags                    JSONB DEFAULT '{}',
    state                   TEXT NOT NULL DEFAULT 'UNKNOWN',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    start_time              TIMESTAMPTZ,
    end_time                TIMESTAMPTZ,
    outputs                 JSONB DEFAULT '{}',
    work_dir                TEXT,
    external_id             TEXT
);

CREATE INDEX idx_wes_runs_state ON wes_runs(state);
CREATE INDEX idx_wes_runs_created_at ON wes_runs(created_at DESC);

CREATE TABLE wes_run_log (
    run_id       TEXT NOT NULL REFERENCES wes_runs(run_id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    cmd          TEXT[],
    start_time   TIMESTAMPTZ,
    end_time     TIMESTAMPTZ,
    stdout_url   TEXT,
    stderr_url   TEXT,
    exit_code    INT,
    PRIMARY KEY (run_id)
);

CREATE TABLE wes_task_logs (
    id           BIGSERIAL PRIMARY KEY,
    run_id       TEXT NOT NULL REFERENCES wes_runs(run_id) ON DELETE CASCADE,
    task_id      TEXT NOT NULL,
    name         TEXT NOT NULL,
    cmd          TEXT[],
    start_time   TIMESTAMPTZ,
    end_time     TIMESTAMPTZ,
    stdout_url   TEXT,
    stderr_url   TEXT,
    exit_code    INT,
    UNIQUE (run_id, task_id)
);

CREATE INDEX idx_wes_task_logs_run_id ON wes_task_logs(run_id);
