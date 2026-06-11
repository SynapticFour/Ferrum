# Reference genomes in Ferrum

Ferrum treats reference genomes as **pluggable metadata** in a registry — never hard-coded to GRCh38. Actual FASTA files are **not bundled or auto-downloaded**; operators load them via DRS ingest and associate them with registry entries.

## Registry

| ID | Organism | Scope | Default |
|----|----------|-------|---------|
| `GRCh38` | Homo_sapiens | Global | yes |
| `T2T-CHM13` | Homo_sapiens | Global | no |
| `H3Africa_v1` | Homo_sapiens | African pangenome | no |
| `AWI-GEN_panel` | Homo_sapiens | African pangenome | no |
| `Pf3D7_v3` | Plasmodium_falciparum | Pathogen (5833) | no |
| `MTB_H37Rv` | Mycobacterium_tuberculosis | Pathogen (83332) | no |

Seeded on migration in PostgreSQL and SQLite (`reference_genomes` table).

## HTTP API

Base path: `/api/v1/references` (gateway)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/references` | List all registered references |
| GET | `/api/v1/references/{id}` | Get one entry (includes DRS IDs if loaded) |
| POST | `/api/v1/references` | Register a new reference (metadata only) |
| PUT | `/api/v1/references/{id}/load` | Associate ingested FASTA/index DRS objects |

### Load a reference FASTA

1. Ingest the FASTA via `/api/v1/ingest/upload` (or Lab Kit).
2. Note the returned `drs_object_id`.
3. Associate it:

```bash
curl -X PUT "http://localhost:8080/api/v1/references/H3Africa_v1/load" \
  -H "Content-Type: application/json" \
  -d '{"fasta_drs_id": "your-fasta-drs-id"}'
```

Optional `.fai` index:

```json
{"fasta_drs_id": "...", "index_drs_id": "..."}
```

## WES reference mismatch warnings

When submitting a WES run with `reference_genome: "GRCh38"` (or leaving it unset, using the default global reference) and input DRS objects suggest **African population origin** (description/metadata, `geo_origin`, or `population_scope` in workflow params), Ferrum returns a **non-blocking warning**:

```json
{
  "run_id": "...",
  "warnings": [{
    "code": "REFERENCE_MISMATCH",
    "message": "Input data may have African population origin. Consider using H3Africa_v1 or AWI-GEN_panel for improved variant calling accuracy.",
    "reference_used": "GRCh38",
    "suggested_alternatives": ["H3Africa_v1", "AWI-GEN_panel"]
  }]
}
```

The run still executes.

## Beacon integration

Pathogen Beacon queries include `meta.referenceGenome` when a matching registry entry exists (by `assemblyId` or `organism` filter).

## Contributing a new entry

1. Add metadata via `POST /api/v1/references` (or propose a migration seed for widely used references).
2. Document organism, population scope, and authoritative source URL.
3. Do **not** commit multi-gigabyte FASTA files to the Ferrum repository.

See also [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) and ADR-016 in [DECISIONS.md](../DECISIONS.md).
