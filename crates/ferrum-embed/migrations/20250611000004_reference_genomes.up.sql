-- Pluggable reference genome registry (metadata only; FASTA loaded via ingest).

CREATE TABLE IF NOT EXISTS reference_genomes (
    id                  TEXT PRIMARY KEY,
    display_name        TEXT NOT NULL,
    organism            TEXT NOT NULL,
    population_scope    TEXT NOT NULL,
    source_url          TEXT,
    fasta_drs_id        TEXT REFERENCES drs_objects(id),
    index_drs_id        TEXT REFERENCES drs_objects(id),
    is_default          INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_reference_genomes_organism ON reference_genomes(organism);
CREATE INDEX IF NOT EXISTS idx_reference_genomes_default ON reference_genomes(is_default);

INSERT OR IGNORE INTO reference_genomes (id, display_name, organism, population_scope, source_url, is_default) VALUES
    ('GRCh38', 'GRCh38', 'Homo_sapiens', 'Global', 'https://www.ncbi.nlm.nih.gov/grc/human/data', 1),
    ('T2T-CHM13', 'T2T-CHM13', 'Homo_sapiens', 'Global', 'https://github.com/nahlgren/t2t-chm13', 0),
    ('H3Africa_v1', 'H3Africa v1', 'Homo_sapiens', 'AfricanPangenome', 'https://www.internationalgenome.org/data-portal/data-collection/h3africa', 0),
    ('AWI-GEN_panel', 'AWI-GEN panel', 'Homo_sapiens', 'AfricanPangenome', NULL, 0),
    ('Pf3D7_v3', 'Plasmodium falciparum 3D7 v3', 'Plasmodium_falciparum', 'Pathogen:5833', 'https://plasmoDB.org/', 0),
    ('MTB_H37Rv', 'Mycobacterium tuberculosis H37Rv', 'Mycobacterium_tuberculosis', 'Pathogen:83332', NULL, 0);
