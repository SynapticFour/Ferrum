# Field sync queue

Design reference for **ADR-019** / **Phase 4 (T4)**. Edge nodes queue DRS objects for upstream sync when connectivity returns.

## Problem

Researchers ingest sequencing data offline on a Raspberry Pi or edge SBC. Objects live in local SQLite + filesystem storage. When VSAT, mobile tether, or a visit to a connected site provides bandwidth, operators upload selected objects (and linked ferrum-meta bundles) to a hub without re-running MinION ingest.

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

## CLI

```bash
ferrum sync status
ferrum sync enqueue --all-local --target https://hub.example.org
ferrum sync enqueue --object-id <drs-id> --target https://hub.example.org
ferrum sync push --target https://hub.example.org
ferrum sync push --dry-run
ferrum sync export --output /media/usb/field-bundle.tar.gz [--policy outbreak-policy]
```

## HTTP API (optional)

When DRS is configured on the gateway:

```http
GET /api/v1/sync/status
POST /api/v1/sync/enqueue?object_id=…&target=…
POST /api/v1/sync/enqueue?all_local=true&target=…
```

## Config (`[sync]`)

```toml
[sync]
default_target_url = "https://hub.example.org"
encrypt_on_push = false
require_metadata_ref = false
allowed_duo_codes = []
allowed_consent_types = []
outbreak_policy_on_export = "default"
register_on_push = true   # ga4gh-infra service registry when online
```

## Upload behaviour

1. Operator runs `sync push` when link is available (no background daemon by default).
2. For each `pending` item, CLI streams object bytes via hub `/api/v1/ingest/upload` (multipart) or chunked resume.
3. Optional `[sync] encrypt_on_push = true` re-wraps plaintext objects in Crypt4GH before upload (future).
4. Successful push → `state = completed`, append `residency_audit` entry `sync_push_completed`.
5. Failure → `state = failed`, retain `bytes_sent` + `resume_token` for retry.

## Hub conflict policy

See [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md): duplicate sample → 409; Edge does not auto-merge.

## Sneakernet export

`ferrum sync export` builds a gzip tar: `manifest.json`, `objects/`, `meta/`, `audit/residency_slice.json`, optional GISAID package.

## Related

- [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md)
- [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md)
- **Solum Kenya / H4:** when Edge `[solum]` consent teeth are enabled under `kenya-dpa`, sync targets must respect KE residency; prefer hub in KE. Policy: [H4-OFFLINE-SYNC-POLICY.md](https://github.com/SynapticFour/Solum/blob/main/docs/H4-OFFLINE-SYNC-POLICY.md). Subject bridge: DRS metadata `solum_subject` = Solum `solum_subject_id` after push.
