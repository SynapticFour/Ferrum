-- Africa surveillance: ONT metrics on DRS, pathogen Beacon annotations, Outbreak Mode audit.

ALTER TABLE drs_objects ADD COLUMN IF NOT EXISTS ont_metrics JSONB;

CREATE TABLE IF NOT EXISTS pathogen_annotations (
    id                  TEXT PRIMARY KEY,
    dataset_id          TEXT REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    drs_object_id       TEXT REFERENCES drs_objects(id) ON DELETE CASCADE,
    organism            TEXT NOT NULL,
    amr_genes           JSONB NOT NULL DEFAULT '[]',
    serotype            TEXT,
    virulence_factors   JSONB NOT NULL DEFAULT '[]',
    ont_qscore_min      DOUBLE PRECISION,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_organism ON pathogen_annotations(organism);
CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_drs ON pathogen_annotations(drs_object_id);
CREATE INDEX IF NOT EXISTS idx_pathogen_annotations_dataset ON pathogen_annotations(dataset_id);

CREATE TABLE IF NOT EXISTS outbreak_activations (
    id                  TEXT PRIMARY KEY,
    policy_name         TEXT NOT NULL,
    trigger_pathogen    TEXT NOT NULL,
    activated_by        TEXT NOT NULL,
    activated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deactivated_at      TIMESTAMPTZ,
    deactivated_by      TEXT,
    deactivation_reason TEXT,
    active              BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_outbreak_activations_policy ON outbreak_activations(policy_name);
CREATE INDEX IF NOT EXISTS idx_outbreak_activations_active ON outbreak_activations(active);

CREATE TABLE IF NOT EXISTS outbreak_audit (
    id                  BIGSERIAL PRIMARY KEY,
    policy_name         TEXT NOT NULL,
    action              TEXT NOT NULL,
    actor               TEXT NOT NULL,
    recipient           TEXT,
    pathogen            TEXT,
    drs_object_id       TEXT,
    query_summary       TEXT,
    details             JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_outbreak_audit_policy ON outbreak_audit(policy_name);
CREATE INDEX IF NOT EXISTS idx_outbreak_audit_created ON outbreak_audit(created_at);

CREATE TABLE IF NOT EXISTS outbreak_download_approvals (
    drs_object_id       TEXT NOT NULL REFERENCES drs_objects(id) ON DELETE CASCADE,
    policy_name         TEXT NOT NULL,
    approved_by         TEXT NOT NULL,
    recipient           TEXT NOT NULL,
    approved_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (drs_object_id, policy_name, recipient)
);
