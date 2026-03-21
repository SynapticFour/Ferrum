# Ingest API for Ferrum Lab Kit

**Audience:** [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) and other **non-interactive** clients. Ferrum owns GA4GH DRS semantics, storage, and optional Crypt4GH; Lab Kit only configures backends and calls these endpoints.

## Overview

| Path | Purpose |
|------|---------|
| `POST /api/v1/ingest/register` | Register **URLs** or **existing blobs** (metadata + DRS access methods; no mandatory copy). |
| `POST /api/v1/ingest/upload` | Multipart **upload** into configured object storage; optional **Crypt4GH** (Ferrum node key). |
| `GET /api/v1/ingest/jobs/{job_id}` | **Job** status and result/error payload (idempotency + auditing). |

These routes are mounted on the **ferrum-gateway** when DRS is enabled (same auth stack as the rest of the gateway).

Legacy GA4GH-adjacent ingest remains under `POST /ga4gh/drs/v1/ingest/*` with Ferrum’s classic `{ "error", "message" }` error shape. **`/api/v1/ingest/*` uses structured errors** for scripting:

```json
{
  "code": "validation_error",
  "message": "items must be non-empty",
  "details": null
}
```

## Authentication

Same as other gateway routes:

- **Demo / dev:** default middleware may attach a demo user when `FERRUM_AUTH__REQUIRE_AUTH` is not `true`.
- **Production:** set `FERRUM_AUTH__REQUIRE_AUTH=true` and configure JWT / JWKS per [INSTALLATION.md](INSTALLATION.md). Pass `Authorization: Bearer <token>`.

Lab Kit should read tokens from env (e.g. `FERRUM_TOKEN`), a file, or OIDC-provided credentials—Ferrum does not prompt interactively on these paths.

## Configuration (Ferrum, not Lab Kit)

```toml
[ingest]
# When true, multipart upload encrypts with Crypt4GH if the client omits the `encrypt` field.
default_encrypt_upload = false
# Optional cap (bytes). Unset or 0 → default 1 GiB.
# max_upload_bytes = 1073741824
```

Env overlay: `FERRUM_INGEST__DEFAULT_ENCRYPT_UPLOAD`, `FERRUM_INGEST__MAX_UPLOAD_BYTES`.

**Crypt4GH (upload encrypt):**

- Set `[encryption].crypt4gh_key_dir` (or `FERRUM_ENCRYPTION__CRYPT4GH_KEY_DIR`) with `{crypt4gh_master_key_id}.pub` / `.sec`.
- Encryption uses the **node public key** as recipient; ciphertext is stored under `drs/<ulid>`. Decryption for `GET …/stream` uses the same node private key (see [CRYPT4GH.md](CRYPT4GH.md)).

**Storage:**

- `storage.backend = "s3"` → S3-compatible `put_bytes`; DRS `storage_references.storage_backend` is the configured backend name (e.g. `s3`).
- Otherwise the gateway uses **`LocalStorage`** under `storage.base_path` or `./ferrum-blobs` by default, and records `storage.backend` in DRS (e.g. `local`).

## Idempotency

Include **`client_request_id`** (opaque string, unique per logical operation from the client):

- **Register:** JSON field `client_request_id` on `POST /register`.
- **Upload:** multipart field `client_request_id`.

If the same `client_request_id` was already processed, Ferrum returns the **existing job** (`job_id`, `status`, `result` / `error`) without duplicating work. Concurrent duplicates rely on a DB unique index and resolve to one job.

## `POST /api/v1/ingest/register`

**Body (JSON):**

```json
{
  "client_request_id": "optional-stable-id",
  "workspace_id": "optional",
  "items": [
    {
      "kind": "url",
      "url": "https://example.org/data.bam",
      "name": "optional",
      "mime_type": "optional",
      "derived_from": ["drs://localhost/parent-id"]
    },
    {
      "kind": "existing_object",
      "storage_backend": "s3",
      "storage_key": "my-bucket/prefix/object.dat",
      "size": 1048576,
      "name": "display name",
      "description": "optional",
      "mime_type": "optional",
      "is_encrypted": false,
      "checksums": [{ "type": "sha-256", "checksum": "..." }]
    }
  ]
}
```

- **`url`:** SSRF-checked; creates a DRS object with `storage_backend: url` (no blob copy).
- **`existing_object`:** **Register / index only** — creates DRS metadata pointing at the given `storage_backend` + `storage_key`. Ferrum does **not** verify that credentials or cluster visibility allow access; operators must align IAM/network with their deployment. Use `kind: "url"` for HTTPS references, not `existing_object` with `storage_backend: url`.

**Response (200):**

```json
{
  "job_id": "01JQ…",
  "status": "succeeded",
  "job_type": "register",
  "result": {
    "object_ids": ["…"],
    "self_uris": ["drs://hostname/id", "…"]
  }
}
```

On validation failure after the job row is created, `status` becomes `failed` and `error` holds a JSON error descriptor; the HTTP response body still uses the structured `code` / `message` form.

## `POST /api/v1/ingest/upload`

**Multipart fields** (same as `POST /ga4gh/drs/v1/ingest/file` plus optional idempotency):

| Field | Required | Description |
|-------|----------|-------------|
| `file` | yes | File bytes |
| `name` | no | Object name |
| `mime_type` | inferred from part if omitted | |
| `encrypt` | no | `true` / `1` → Crypt4GH with node pubkey; if omitted, `[ingest].default_encrypt_upload` applies |
| `expected_sha256` | no | Reject if mismatch |
| `workspace_id` | no | Requires authenticated workspace editor/owner |
| `client_request_id` | no | Idempotency key |

**Response (200):** job envelope with `result.object_ids`, `result.self_uris`, `result.size`.

## `GET /api/v1/ingest/jobs/{job_id}`

Returns the same job shape as the POST responses: `job_id`, `status` (`running` \| `succeeded` \| `failed`), `job_type`, optional `result`, optional `error`.

## Example: curl

```bash
BASE=http://localhost:8080
HDR=(-H "Authorization: Bearer $TOKEN")  # omit if demo auth

curl -sS "${HDR[@]}" "$BASE/api/v1/ingest/register" \
  -H "Content-Type: application/json" \
  -d '{"client_request_id":"demo-1","items":[{"kind":"url","url":"https://example.com/f.txt","name":"demo"}]}'

curl -sS "${HDR[@]}" "$BASE/api/v1/ingest/upload" \
  -F "client_request_id=demo-upload-1" \
  -F "file=@./README.md;type=text/plain"

curl -sS "${HDR[@]}" "$BASE/api/v1/ingest/jobs/<job_id_from_response>"
```

Verify DRS: `GET $BASE/ga4gh/drs/v1/objects/<object_id>`.

A minimal end-to-end script (no auth header; use demo gateway) lives at [`scripts/demo_ingest_lab_kit.sh`](../scripts/demo_ingest_lab_kit.sh).

## Tests & coverage

- **Unit tests (CI):** `ferrum-drs` — JSON shape of structured errors and deserialization of `POST /register` bodies (`api_v1` test module). `ferrum-core` — `[ingest]` effective upload size defaults.
- **HelixTest:** does not yet call `/api/v1/ingest/*`; see [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md).
- **Manual / demo:** [`scripts/demo_ingest_lab_kit.sh`](../scripts/demo_ingest_lab_kit.sh) against a running gateway.

## Semver / stability

`/api/v1/ingest/*` is intended as a **stable, versioned** contract for Lab Kit. Breaking changes should bump the path (e.g. `/api/v2/ingest/…`) or be gated behind explicit deprecation.

## Design notes (resolved for v1)

| Topic | Choice |
|-------|--------|
| Service placement | Routes on **ferrum-gateway** (no separate ingest microservice). |
| Federation | **Single-site** DRS hostname per deployment; no cross-site federation in this API v1. |
| Register checksums | **Optional** on `existing_object`; policy can be tightened in config later. |
| Multi-tenant | **Workspace** optional on requests; broader org templates remain operator-defined (buckets, IAM). |

---

*[← Documentation index](README.md)*
