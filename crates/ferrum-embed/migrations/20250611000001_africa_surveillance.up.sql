-- Africa surveillance (SQLite embed): ONT metrics, pathogen annotations, outbreak mode.

ALTER TABLE drs_objects ADD COLUMN ont_metrics TEXT;

CREATE TABLE IF NOT EXISTS pathogen_annotations (
    id                  TEXT PRIMARY KEY,
    dataset_id          TEXT REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    drs_object_id       TEXT REFERENCES drs_objects(id) ON DELETE CASCADE,
    organism            TEXT NOT NULL,
    amr_genes           TEXT NOT NULL DEFAULT '[]',
    serotype            TEXT,
    virulence_factors   TEXT NOT NULL DEFAULT '[]',
    ont_qscore_min      REAL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_organism ON pathogen_annotations(organism);
CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_drs ON pathogen_annotations(drs_object_id);
CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_dataset ON pathogen_annotations(dataset_id);

CREATE TABLE IF NOT EXISTS outbreak_activations (
    id                  TEXT PRIMARY KEY,
    policy_name         TEXT NOT NULL,
    trigger_pathogen    TEXT NOT NULL,
    activated_by        TEXT NOT NULL,
    activated_at        TEXT NOT NULL DEFAULT (datetime('now')),
    deactivated_at      TEXT,
    deactivated_by      TEXT,
    deactivation_reason TEXT,
    active              INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_outbreak_activations_policy ON outbreak_activations(policy_name);
CREATE INDEX IF NOT EXISTS idx_outbreak_activations_active ON outbreak_activations(active);

CREATE TABLE IF NOT EXISTS outbreak_audit (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_name         TEXT NOT NULL,
    action              TEXT NOT NULL,
    actor               TEXT NOT NULL,
    recipient           TEXT,
    pathogen            TEXT,
    drs_object_id       TEXT,
    query_summary       TEXT,
    details             TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_outbreak_audit_policy ON outbreak_audit(policy_name);
CREATE INDEX IF NOT EXISTS idx_outbreak_audit_created ON outbreak_audit(created_at);

CREATE TABLE IF NOT EXISTS outbreak_download_approvals (
    drs_object_id       TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    policy_name         TEXT NOT NULL,
    approved_by         TEXT NOT NULL,
    recipient           TEXT NOT NULL,
    approved_at         TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (drs_object_id, policy_name, recipient)
);
