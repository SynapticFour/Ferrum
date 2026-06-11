# Outbreak Mode

Outbreak Mode enables **policy-based, auditable emergency sharing** during declared pathogen outbreaks. It is **opt-in** and **disabled by default**.

## Why policy-based access?

After COVID-19, countries that shared genomic data quickly sometimes faced travel restrictions and punitive responses. Outbreak Mode lets operators:

1. **Activate a named policy** when an outbreak is declared.
2. Grant **emergency Beacon yes/no access** to pre-approved recipients (WHO, Africa CDC, etc.) for a specific pathogen.
3. Keep **full DRS downloads** behind explicit per-object approval.
4. Log **every action** to an append-only `outbreak_audit` table.

## Configuration

```toml
[outbreak]
enabled = false  # must be explicitly true

[[outbreak.policies]]
name = "mpox_who_emergency"
trigger_pathogen = "Monkeypox_virus"
emergency_recipients = ["who.int", "africacdc.org"]
access_level = "beacon_only"   # or "full" (downloads still need approval)
gisaid_auto_package = true
```

| Field | Meaning |
|-------|---------|
| `trigger_pathogen` | Organism name matched against Beacon pathogen filters and DRS tags |
| `emergency_recipients` | Passport issuer domains or explicit recipient IDs |
| `access_level` | `beacon_only` = yes/no Beacon queries bypass DAC; `full` tier still requires download approval |
| `gisaid_auto_package` | Hint for operators to run the GISAID CLI (see below) |

## Authentication

| Role | Visa / scope | Capability |
|------|----------------|------------|
| `outbreak_activator` | Passport visa `ferrum:outbreak_activator` | Activate/deactivate policies, approve downloads |
| Emergency recipient | Passport issuer in `emergency_recipients` | Beacon yes/no queries for `trigger_pathogen` while policy active |
| Admin | `ferrum:admin` | Same as activator |

## HTTP API

Base path: `/api/v1/outbreak` (gateway, when `[outbreak] enabled = true` and Beacon/DB configured).

### Activate

```http
POST /api/v1/outbreak/activate
Content-Type: application/json

{
  "policy": "mpox_who_emergency",
  "activated_by": "user@institution.org"
}
```

Requires `outbreak_activator` or admin.

### Deactivate

```http
POST /api/v1/outbreak/deactivate
Content-Type: application/json

{
  "policy": "mpox_who_emergency",
  "reason": "outbreak contained"
}
```

Emergency access is revoked immediately. Deactivation is audited.

### Approve download

```http
POST /api/v1/outbreak/approve-download/{drs_id}
Content-Type: application/json

{
  "recipient": "who.int",
  "approved_by": "dac-chair@institution.org"
}
```

Required even under `access_level = "full"` for DRS object download.

## Audit trail

All activations, deactivations, Beacon queries under emergency access, and download approvals append rows to `outbreak_audit`. There is **no delete API** for audit rows.

## GISAID submission package (CLI)

Build a GISAID EpiCoV-style archive (CSV + FASTA inside `.tar.gz`):

```bash
ferrum outbreak package \
  --policy mpox_who_emergency \
  --output ./gisaid_pkg.tar.gz
```

Optional: `--config /etc/ferrum/config.toml`

The command collects DRS objects tagged with the policy’s `trigger_pathogen` via `pathogen_annotations` and writes:

- `gisaid_submission.csv` — EpiCoV column headers
- `sequences.fasta` — consensus sequences
- `metadata.txt` — policy name and entry count

Format follows the public GISAID submission spec ([gisaid.org/submission](https://www.gisaid.org/submission/)).

## Interaction with Beacon pathogen filters

When Outbreak Mode is active and a recipient matches `emergency_recipients`, Beacon queries with `organism` matching `trigger_pathogen` bypass normal DAC checks for **boolean/count** responses. Coordinate-based human variant queries are unaffected unless pathogen filters are also supplied.

## Laptop / offline deployments

Outbreak tables migrate on both PostgreSQL and SQLite (embed). Policies remain config-driven; activation state lives in the database.

---

*[← Africa deployment](AFRICA-DEPLOYMENT.md) · [Documentation index](README.md)*
