# Field sync queue

Design reference for **ADR-019**. Edge nodes queue DRS objects for upstream sync when connectivity returns.

## Problem

Researchers ingest sequencing data offline on a Raspberry Pi or edge SBC. Objects live in local SQLite + filesystem storage. When VSAT, mobile tether, or a visit to a connected site provides bandwidth, operators must upload selected objects (and linked ferrum-meta bundles) to a hub without re-running MinION ingest.

## Queue model

SQLite table `sync_queue` (edge / embedded backend):

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PK | ULID |
| `object_id` | TEXT | DRS object to sync |
| `target_url` | TEXT | Hub base URL or DRS endpoint |
| `state` | TEXT | `pending`, `in_progress`, `completed`, `failed` |
| `bytes_total` | INTEGER | Object size at enqueue time |
| `bytes_sent` | INTEGER | Progress for resume |
| `resume_token` | TEXT | Links to `transfer_checkpoints` when used |
| `crypt4gh` | BOOLEAN | Object stored encrypted |
| `metadata_ref` | TEXT | Optional ferrum-meta submission alias |
| `created_at` | TEXT | ISO8601 |
| `last_attempt_at` | TEXT | ISO8601 |
| `error_message` | TEXT | Last failure (truncated) |

## CLI (planned)

```bash
ferrum sync status
ferrum sync enqueue --all-local
ferrum sync enqueue --object-id <drs-id> --target https://hub.example.org
ferrum sync push --target https://hub.example.org
ferrum sync push --dry-run
```

Phase 1 (current): ADR + this document + CLI stub returning "not yet implemented".  
Phase 4 (maturity plan): full push adapter with resume and residency audit.

## Upload behaviour

1. Operator runs `sync push` when link is available (no background daemon by default).
2. For each `pending` item, gateway streams object bytes via existing DRS multipart/chunked paths.
3. Optional `[sync] encrypt_on_push = true` re-wraps plaintext objects in Crypt4GH before upload.
4. Successful push → `state = completed`, append `residency_audit` entry `sync_push_completed`.
5. Failure → `state = failed`, retain `bytes_sent` + `resume_token` for retry.

## Hub conflict policy

Document only (hub-side): duplicate `sample_id` / alias → reject with 409 or accept with version suffix. Edge node does not auto-merge.

## Related

- [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md)
- [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md)
- [FIELD-MATURITY-PLAN.md](FIELD-MATURITY-PLAN.md)
