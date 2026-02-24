-- Cohort Browser: named versioned sample collections with phenotype and DRS links
CREATE TABLE IF NOT EXISTS cohorts (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    owner_sub       TEXT NOT NULL,
    workspace_id    TEXT,
    version         INTEGER NOT NULL DEFAULT 1,
    is_frozen       BOOLEAN NOT NULL DEFAULT FALSE,
    sample_count    INTEGER NOT NULL DEFAULT 0,
    tags            JSONB NOT NULL DEFAULT '[]',
    filter_criteria JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT now(),
    updated_at      TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS cohort_samples (
    id              TEXT PRIMARY KEY,
    cohort_id       TEXT NOT NULL REFERENCES cohorts(id) ON DELETE CASCADE,
    sample_id       TEXT NOT NULL,
    drs_object_ids  JSONB NOT NULL DEFAULT '[]',
    phenotype       JSONB NOT NULL DEFAULT '{}',
    added_at        TIMESTAMPTZ DEFAULT now(),
    added_by        TEXT NOT NULL,
    UNIQUE(cohort_id, sample_id)
);

CREATE TABLE IF NOT EXISTS phenotype_schema (
    id           TEXT PRIMARY KEY,
    field_name   TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    field_type   TEXT NOT NULL,
    ontology     TEXT,
    required     BOOLEAN DEFAULT FALSE,
    description  TEXT
);

INSERT INTO phenotype_schema (id, field_name, display_name, field_type, ontology) VALUES
    ('ps-01', 'age_at_enrollment',  'Age at Enrollment',   'number',   NULL),
    ('ps-02', 'sex',                'Biological Sex',      'string',   NULL),
    ('ps-03', 'diagnosis',          'Primary Diagnosis',   'ontology', 'ICD10'),
    ('ps-04', 'ancestry',           'Genetic Ancestry',    'string',   NULL),
    ('ps-05', 'tissue_type',        'Tissue Type',         'ontology', 'UBERON'),
    ('ps-06', 'sequencing_type',    'Sequencing Type',     'string',   NULL),
    ('ps-07', 'sequencing_depth',   'Sequencing Depth',    'number',   NULL),
    ('ps-08', 'library_prep',       'Library Prep Kit',    'string',   NULL),
    ('ps-09', 'tumor_normal',       'Tumor/Normal',        'string',   NULL),
    ('ps-10', 'cohort_enrollment',  'Cohort Enrollment',   'date',     NULL)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS cohort_versions (
    id          TEXT PRIMARY KEY,
    cohort_id   TEXT NOT NULL REFERENCES cohorts(id),
    version     INTEGER NOT NULL,
    snapshot    JSONB NOT NULL,
    created_at  TIMESTAMPTZ DEFAULT now(),
    created_by  TEXT NOT NULL,
    note        TEXT,
    UNIQUE(cohort_id, version)
);

CREATE INDEX IF NOT EXISTS idx_cohort_samples_cohort ON cohort_samples(cohort_id);
CREATE INDEX IF NOT EXISTS idx_cohort_samples_phenotype ON cohort_samples USING GIN(phenotype);
CREATE INDEX IF NOT EXISTS idx_cohorts_owner ON cohorts(owner_sub);
CREATE INDEX IF NOT EXISTS idx_cohorts_updated ON cohorts(updated_at DESC);
