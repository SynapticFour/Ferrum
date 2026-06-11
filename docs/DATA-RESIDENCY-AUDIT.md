# Data Residency Audit Log

Ferrum maintains an **append-only, cryptographically chained** audit log of data movement and sensitive queries. It supports compliance evidence for African deployments where data sovereignty must be demonstrable.

## Properties

- **Append-only** — rows cannot be updated or deleted via SQL triggers or HTTP API.
- **Chained** — each entry includes `prev_hash` (SHA-256 of the prior entry) and `entry_hash` (SHA-256 of canonical JSON for this row).
- **Dual backend** — PostgreSQL and SQLite (Laptop Mode).

## Schema

Table `residency_audit`:

| Column | Description |
|--------|-------------|
| `event_type` | e.g. `data_accessed`, `data_downloaded`, `data_uploaded`, `beacon_query`, `peer_query_sent`, `outbreak_activated` |
| `drs_id` | Optional DRS object id |
| `requester` | Passport `sub` or client IP |
| `destination` | Peer name, issuer, or IP |
| `data_left_node` | `true` when bytes leave the node (downloads) |
| `bytes_transferred` | Optional byte count |
| `prev_hash` / `entry_hash` | Chain links |

## HTTP API

### Query range

```http
GET /api/v1/audit/residency?from=2026-01-01T00:00:00Z&to=2026-12-31T23:59:59Z
```

Response includes `entries` and `chain_valid`.

### Verify chain

```http
GET /api/v1/audit/residency/verify
```

```json
{
  "chain_valid": true,
  "entry_count": 1042,
  "first_timestamp": "...",
  "last_timestamp": "...",
  "last_hash": "..."
}
```

`DELETE`, `PUT`, and `POST` on `/residency` return **405 Method Not Allowed**.

## Integration points

| Event | `event_type` | `data_left_node` |
|-------|--------------|------------------|
| DRS access URL | `data_accessed` | false |
| DRS `/stream` | `data_downloaded` | true |
| Beacon query | `beacon_query` | false |
| Federation fan-out | `peer_query_sent` | false |

Outbreak activations should also be logged (see [OUTBREAK-MODE.md](OUTBREAK-MODE.md)).

## Verification workflow

1. Export entries via the query API (or DB backup).
2. Call `/verify` after incidents or audits.
3. Any tampering with historical rows breaks `chain_valid`.

Genesis hash: 64 zero hex digits (`000…000`).
