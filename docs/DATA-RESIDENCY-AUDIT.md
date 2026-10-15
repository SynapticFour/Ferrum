# Data Residency Audit Log

Ferrum keeps hash-chained rows for the residency events that are recorded. Each row stores `prev_hash` and a SHA-256 of a canonical JSON form of that row. While the database triggers remain, those rows are not updated or deleted through SQL. The log is not a list of every data movement, and it does not prove that data stayed inside an institution.

## Properties

- **Append-only** — rows cannot be updated or deleted via SQL triggers or HTTP API.
- **Chained** — each entry includes `prev_hash` (SHA-256 of the prior entry) and `entry_hash` (SHA-256 of canonical JSON for this row). The previous hash is read and the new row is inserted in one transaction, so concurrent appends keep a single chain.
- **Dual backend** — PostgreSQL and SQLite (Laptop Mode).

## Schema

Table `residency_audit`:

| Column | Description |
|--------|-------------|
| `event_type` | e.g. `data_accessed`, `data_downloaded`, `data_uploaded`, `collection_recorded`, `beacon_query`, `peer_query_sent`, `outbreak_activated` |
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

Response includes `entries` and `chain_valid`. Query parameters are `from` and `to` only. There is no page size and no cursor. A caller with no admin claim sees only rows whose `requester` is their own `sub`. An admin sees the range. This route uses the bearer claims when they are present. It is not the unauthenticated verify route below.

### Verify chain

```http
GET /api/v1/audit/residency/verify
```

This route does not read `Authorization`. It stays that way (ADR-027). The body is chain metadata and not the log:

| Field | Meaning |
|-------|---------|
| `chain_valid` | Whether each stored row's hash matches the previous row |
| `entry_count` | How many rows were hashed |
| `first_timestamp` | Timestamp of the first row, if any |
| `last_timestamp` | Timestamp of the last row, if any |
| `last_hash` | Hash of the last row |

There is no `entries` array, no `requester`, no `drs_id`, and no event payload. `entry_count` and the two timestamps say that the log is non-empty and when it spans. They do not name a person or an object.

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
| ONT multipart ingest | `data_uploaded` | false, when a residency log is configured and the insert succeeds. Uses `append_warn`: a failed insert is logged and the object can still be stored. That path is not fail-closed. |
| `POST /api/v1/ingest/register` | `data_uploaded` | false. When a residency log is configured, uses `append`. A failed insert fails the request and the new object row is deleted. If that delete also fails, the request is still an error and the object may remain. There is no setting that makes register ignore the error. When no log is configured, no row is written. This path does not use `append_warn`. |
| ONT ingest with provenance | `collection_recorded` | false (`destination` holds JSON: collector, geo, metadata_ref). Uses `append_warn`. |
| Beacon query | `beacon_query` | false |
| Federation fan-out | `peer_query_sent` | false |

Outbreak activations should also be logged (see [OUTBREAK-MODE.md](OUTBREAK-MODE.md)).

## Verification workflow

1. Export entries via the query API (or DB backup).
2. Call `/verify` after incidents or audits.
3. `chain_valid` covers the rows that are still in the table. While the triggers exist, UPDATE and DELETE are rejected. After the triggers are dropped, a holder of the database file can replace the table with a new chain that verifies. A filtered query can report `chain_valid` false while the full chain verifies. `data_left_node` is the boolean stored on the row. It is not a check against an institution boundary.

## When an audit write fails

`append` returns the error. Callers that propagate it fail the request. That is fail-closed. There is no configuration flag that turns a failed `append` into success.

`append_warn` logs the error and returns. The caller continues. ONT multipart ingest uses `append_warn`. `POST /api/v1/ingest/register` does not use `append_warn`.

Genesis hash: 64 zero hex digits (`000…000`).
