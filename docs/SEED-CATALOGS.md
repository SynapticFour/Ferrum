# Seed catalogs — local vs Fly pilot

Ferrum uses **two operator seed paths** with **different DRS display names**. Testers on Fly should not expect `make seed-pilot` object names; local smoke scripts should not assume Fly GIAB-style filenames.

## Quick reference

| | **Local** (`make seed-pilot`) | **Fly** (`./pilot.sh seed all`) |
|---|-------------------------------|----------------------------------|
| **When** | After `make up-tes` / `make up-pilot` | After Fly deploy + Passport in `.env` |
| **Command** | `make seed-pilot` | `cd synapticfour-business/.../pilot-deploy && ./pilot.sh seed all` |
| **Verify** | `make smoke-pilot` | `./scripts/pilot-smoke.sh` or `Ferrum/scripts/seed-pilot-remote.sh` |
| **Workspace** | `demo-workspace-01` | Created by seed (e.g. “Pasteur Pilot”) |
| **Cohort / sample** | `demo-cohort-01` / `pilot-demo-01` | May differ — Fly seed focuses on DRS + Beacon |

## DRS object names

### Local (`scripts/seed-pilot-demo.sh`)

| DRS name | Role |
|----------|------|
| `Pilot demo VCF (MinIO)` | Germline demo input |
| `Pilot demo BAM (MinIO)` | Aligned reads |
| `Pilot demo BAM index (MinIO)` | BAM index |
| `Pilot demo reference FASTA (MinIO)` | chr22 ref slice |
| `Pilot demo truth VCF (MinIO)` | Truth for QC |

### Fly (`pilot-deploy/seed/data/manifest.yaml`)

DRS **name** = manifest `name` field (filename):

| DRS name | Role |
|----------|------|
| `na12878_slice.bam` | BAM slice |
| `na12878_slice.bam.bai` | BAM index |
| `truth_slice.vcf.gz` | Truth VCF |
| `truth_slice.vcf.gz.tbi` | VCF index |
| `ref_slice.fa` | Reference FASTA |
| `ref_slice.fa.fai` | FASTA index |

Beacon queries use **GRCh37 chr22:2000** (synthetic GIAB-style slice) — same biology as local demo, different catalog labels.

## Why two catalogs?

- **Local** names are human-readable for `make smoke-pilot` and UI empty states during Docker development.
- **Fly** seed ingests from `manifest.yaml` using filenames as stable storage keys; operator bundle stays small (synthetic generate, no large binaries in git).

## Tester guidance

- **Fly testers:** If Data is empty, ask the operator to run `./pilot.sh seed all` — do **not** run `make seed-pilot` (that is local Docker only).
- **Operators:** After seed, run `./scripts/pilot-smoke.sh` or `FERRUM_PASSPORT_JWT=… BASE_URL=https://pasteur-pilot-ferrum.fly.dev ./scripts/seed-pilot-remote.sh`.

## Related docs

- [PASTEUR-PILOT.md](PASTEUR-PILOT.md) — URLs and local profiles
- [UAT-MATURITY.md](UAT-MATURITY.md) — maturity tracker
- `synapticfour-business/customers/pasteur-tunis/pilot-deploy/seed/README.md` — Fly seed walkthrough
- `synapticfour-business/customers/pasteur-tunis/pilot-deploy/HANDOFF.md` — operator runbook
