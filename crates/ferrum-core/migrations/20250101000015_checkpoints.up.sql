-- Workflow resume and checkpointing: extend wes_runs and add checkpoint/cache tables.

ALTER TABLE wes_runs ADD COLUMN IF NOT EXISTS resumed_from_run_id TEXT REFERENCES wes_runs(run_id);
ALTER TABLE wes_runs ADD COLUMN IF NOT EXISTS checkpoint_enabled   BOOLEAN NOT NULL DEFAULT TRUE;

CREATE TABLE IF NOT EXISTS wes_checkpoints (
    id           TEXT PRIMARY KEY,
    run_id       TEXT NOT NULL REFERENCES wes_runs(run_id) ON DELETE CASCADE,
    task_name    TEXT NOT NULL,
    task_hash    TEXT NOT NULL,
    status       TEXT NOT NULL,
    drs_object_ids JSONB NOT NULL DEFAULT '[]',
    created_at   TIMESTAMPTZ DEFAULT now(),
    UNIQUE(run_id, task_name, task_hash)
);

CREATE TABLE IF NOT EXISTS wes_cache_entries (
    task_hash    TEXT PRIMARY KEY,
    drs_object_ids JSONB NOT NULL,
    hit_count    INTEGER NOT NULL DEFAULT 0,
    last_used_at TIMESTAMPTZ DEFAULT now(),
    created_at   TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_run   ON wes_checkpoints(run_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_hash  ON wes_checkpoints(task_hash);
