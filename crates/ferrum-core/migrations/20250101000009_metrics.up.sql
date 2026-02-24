-- Run metrics and cost summary (WES/TES)
CREATE TABLE IF NOT EXISTS task_metrics (
    id                      TEXT PRIMARY KEY,
    run_id                  TEXT NOT NULL,
    task_id                 TEXT NOT NULL,
    task_name               TEXT NOT NULL,
    started_at              TIMESTAMPTZ,
    finished_at             TIMESTAMPTZ,
    wall_seconds            INTEGER,
    cpu_requested           FLOAT,
    cpu_peak_pct            FLOAT,
    memory_requested_mb     BIGINT,
    memory_peak_mb          BIGINT,
    read_bytes              BIGINT,
    write_bytes             BIGINT,
    exit_code               INTEGER,
    executor                TEXT,
    node_hostname           TEXT,
    samples                 JSONB DEFAULT '[]',
    UNIQUE (run_id, task_id)
);

CREATE INDEX idx_task_metrics_run ON task_metrics(run_id);

CREATE TABLE IF NOT EXISTS run_cost_summary (
    run_id                      TEXT PRIMARY KEY,
    total_wall_seconds          INTEGER,
    total_cpu_seconds           FLOAT,
    total_memory_gb_h           FLOAT,
    peak_memory_mb              BIGINT,
    total_read_gb               FLOAT,
    total_write_gb              FLOAT,
    estimated_cost_usd         FLOAT,
    pricing_config_snapshot     JSONB,
    computed_at                 TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_run_cost_summary_computed ON run_cost_summary(computed_at);
