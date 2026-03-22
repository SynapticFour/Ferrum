# Performance: DRS, Plain vs. Crypt4GH

This guide supports **reproducible benchmarks** (e.g. a separate **Ferrum GA4GH Demo / Benchmark** repo) comparing **plaintext at-rest** and **Crypt4GH at-rest** access paths over the **real GA4GH DRS API** — without private Ferrum internals.

**Related:** [CRYPT4GH.md](CRYPT4GH.md) (threat model, header re-wrap, `/stream` decrypt), [GA4GH.md](GA4GH.md) (endpoints), [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md) (upload + `encrypt`).

---

## Comparable Plain vs. Crypt4GH objects

| Goal | Approach |
|------|-----------|
| **Same logical payload** | Use **identical source bytes** for both arms: e.g. upload the same file twice (or clone checksum after ingest), once with encryption off and once with encryption on. |
| **Plain at-rest** | Ingest with **`is_encrypted: false`** (default) or `encrypt=false` on multipart; `storage_references.is_encrypted = false`. |
| **Crypt4GH at-rest** | Ingest with **`is_encrypted: true`** / `encrypt=true` (and `[ingest].default_encrypt_upload` for `/api/v1/ingest/upload`); node public key must be configured. |
| **Demo stack shortcut (Plain only)** | Seeded object **`microbench-plain-v1`**: **4096** bytes (`P` repeated), **SHA-256** `26b7e40be0bcf3e6667020b3acf6e07faa17585b21b2936305dd6c9ad3860b15`, MinIO key `microbench/plain-v1.bin`, **`GET .../objects/microbench-plain-v1/stream`**. Created by **`deploy/scripts/init-demo.sh`**. There is **no** automatic Crypt4GH twin in init — add one via ingest for apples-to-apples Crypt4GH timing. |

**GA4GH note:** `GET .../access/{access_id}` returns **JSON** with a URL; **`GET .../stream`** returns **raw bytes**. For micro-benchmarks that isolate DRS read/decrypt cost, **`/stream`** is usually the right endpoint once auth allows it.

---

## What costs what (expectations)

| Path | Server work (typical) |
|------|------------------------|
| **Plain `/stream`** | Read from object storage → chunk to HTTP (bounded channel; default read chunk **64 KiB**). |
| **Crypt4GH `/stream`** | Read encrypted blob → **decrypt** (header + ChaCha20-Poly1305 segments) → chunk to HTTP. Expect **more CPU** than plain; wall time also depends on **storage latency** and **TLS**. |
| **Header re-wrap** (client pubkey, non-`/stream` flows) | **O(header size)** — body streamed through; see [CRYPT4GH.md](CRYPT4GH.md). |

Ferrum does **not** implement **HTTP Range** on `/stream` today; clients that read only **N** bytes should stop reading after N on the socket (wall time still includes server work for data sent before cancel).

---

## API behaviour for external timers

1. **`GET /ga4gh/drs/v1/objects/{id}`** — metadata (optional `access_methods`); use to discover `access_id` if needed.
2. **`GET /ga4gh/drs/v1/objects/{id}/access/{access_id}`** — JSON `AccessUrl` (time this separately if you care about presign latency).
3. **`GET /ga4gh/drs/v1/objects/{id}/stream`** — **byte stream**; set `Authorization: Bearer …` if auth is required.

**Ferrum extension (benchmark-friendly, not GA4GH-standard):**

- Response header **`X-Ferrum-DRS-Stream-Path`**: `plaintext` | `crypt4gh_decrypt`  
  Lets scripts classify the path without parsing bodies.

**Structured logs (operators):** target **`ferrum_drs::stream`**, JSON-friendly fields:

- `event = drs.stream.started` — `object_id`, `encrypted`, `declared_size`, `storage_backend`
- `event = drs.stream.finished` — `stream_path`, `bytes_from_storage` (plain) or `bytes_to_client` + `decrypt_ok` (Crypt4GH)

Enable `RUST_LOG=ferrum_drs::stream=info` (or `info` globally) to correlate with client wall times.

---

## Minimal measurement recipes

### curl (wall time, full stream)

```bash
BASE=http://localhost:8080
ID=microbench-plain-v1
# Optional: TOKEN=... ; -H "Authorization: Bearer $TOKEN"
curl -sS -o /dev/null -w 'connect:%{time_connect} starttransfer:%{time_starttransfer} total:%{time_total}\n' \
  "$BASE/ga4gh/drs/v1/objects/$ID/stream"
curl -sSI "$BASE/ga4gh/drs/v1/objects/$ID/stream" | grep -i x-ferrum-drs-stream-path
```

### Python (time to first byte + N bytes)

```python
import time, urllib.request
base, oid = "http://localhost:8080", "microbench-plain-v1"
url = f"{base}/ga4gh/drs/v1/objects/{oid}/stream"
req = urllib.request.Request(url)  # add headers={"Authorization": "Bearer ..."} if needed
t0 = time.perf_counter()
with urllib.request.urlopen(req) as r:
    ttfb = time.perf_counter() - t0
    chunk = r.read(4096)  # or r.read() for full body
print("ttfb_s", ttfb, "read_bytes", len(chunk))
```

Repeat with your **Crypt4GH** object id (same size/checksum plaintext before encryption if you verify fidelity via decrypt).

---

## Pitfalls (fair benchmarks)

| Issue | Mitigation |
|-------|------------|
| **Cold cache / first byte** | Warm up once; report **TTFB** and **full download** separately. |
| **localhost vs. remote storage** | Demo Compose uses **MinIO** beside the gateway; production may use remote S3 — latency differs. |
| **HTTP/2 coalescing / proxies** | Put the gateway behind the same reverse proxy in both arms. |
| **Auth** | Include token fetch in **separate** timings if comparing only DRS. |
| **`/stream` disabled for Crypt4GH** | Set **`encryption.crypt4gh_decrypt_stream = true`** and **`crypt4gh_key_dir`** or Crypt4GH `/stream` returns 4xx. |
| **URL-backed DRS objects** | **`/stream`** requires **`local` / `s3` / `minio`** backend — not `url`-only rows. |

---

## CI: fast DRS /stream regression

Script **`deploy/scripts/ci-drs-microbench-stream.sh`** checks **`microbench-plain-v1`**: HTTP 200, **4096** bytes, expected SHA-256, **`X-Ferrum-DRS-Stream-Path: plaintext`**. Invoked from [`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) so DRS stream regressions fail **before** heavy HelixTest suites.

---

## WES / pipelines (WDL, Nextflow)

See **[WES-WORKFLOW-ENGINES.md](WES-WORKFLOW-ENGINES.md)** for `workflow_type`, **TES-backed** runs, and **Docker workdir** / long-run notes (**[TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md)**).

---

*[← Documentation index](README.md)*
