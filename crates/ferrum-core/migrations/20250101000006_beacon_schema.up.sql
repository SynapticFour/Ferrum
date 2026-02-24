-- GA4GH Beacon v2: datasets, genomic variants, individuals, biosamples.

CREATE TABLE beacon_datasets (
    id              TEXT PRIMARY KEY,
    name            TEXT,
    description     TEXT,
    assembly_id      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE beacon_variants (
    id              BIGSERIAL PRIMARY KEY,
    dataset_id       TEXT NOT NULL REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    chromosome      TEXT NOT NULL,
    start           BIGINT NOT NULL,
    "end"           BIGINT NOT NULL,
    reference       TEXT,
    alternate       TEXT,
    variant_type     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_beacon_variants_dataset ON beacon_variants(dataset_id);
CREATE INDEX idx_beacon_variants_chr_start ON beacon_variants(chromosome, start, "end");

CREATE TABLE beacon_individuals (
    id              TEXT PRIMARY KEY,
    dataset_id       TEXT NOT NULL REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    sex             TEXT,
    phenotypic_features JSONB DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_beacon_individuals_dataset ON beacon_individuals(dataset_id);

CREATE TABLE beacon_biosamples (
    id              TEXT PRIMARY KEY,
    dataset_id       TEXT NOT NULL REFERENCES beacon_datasets(id) ON DELETE CASCADE,
    individual_id   TEXT REFERENCES beacon_individuals(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_beacon_biosamples_dataset ON beacon_biosamples(dataset_id);
