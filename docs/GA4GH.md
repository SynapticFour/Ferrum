# GA4GH standards implementation

This document lists each GA4GH standard implemented by Ferrum: version, specification link, endpoints, auth requirements, Ferrum-specific extensions, and known limitations.

---

## If you already work with GA4GH APIs

These patterns match what many **reference stacks**, **Starter Kit** deployments, and **interop tutorials** in the GA4GH ecosystem use—so Ferrum should feel familiar:

| Convention | How Ferrum maps to it |
|------------|------------------------|
| **Path layout** | APIs live under **`/ga4gh/<product>/<major-version>/…`** on the **gateway** (single host), e.g. `…/ga4gh/drs/v1/…`, `…/ga4gh/wes/v1/…` — the usual prefix pattern for [GA4GH technical standards](https://www.ga4gh.org/) and their published OpenAPI bases. |
| **Service info** | Each product exposes **`GET …/service-info`** (DRS, WES, TES, TRS, htsget reads/variants). The cross-product [**GA4GH Discovery service-info**](https://github.com/ga4gh-discovery/ga4gh-service-info) spec describes the common metadata shape; **removing `/service-info` from the URL** yields the usual **API base** clients expect for that service (see also [Starter Kit — configuring service-info](https://starterkit.ga4gh.org/docs/concepts-and-guides/configuring-service-info)). |
| **Bearer auth & Passports** | Protected routes expect **`Authorization: Bearer <JWT>`** when auth is enabled; **GA4GH Passports / Visas** ([Passport v1](https://github.com/ga4gh-duri/ga4gh-passport-v1)) are validated for dataset and service access—aligned with **AAI** practice in ELIXIR and other nodes. |
| **`drs://` URIs** | Objects use canonical **`drs://<drs_hostname>/<object_id>`** (hostname from Ferrum DRS config, not the full HTTP path). Resolve with **`GET /ga4gh/drs/v1/objects/{id}`** or by passing the URI where the API accepts it. |
| **DRS access vs bytes** | Use **`GET …/access/{access_id}`** for a **JSON** URL (and optional presigned URL on S3/MinIO); use **`GET …/stream`** for **raw bytes**—see [DRS: access vs stream](#drs-get-accessaccess_id-vs-get-stream). |
| **OpenAPI** | The **DRS** crate exposes **Swagger UI** when run with OpenAPI enabled (useful for trying requests next to the spec). Other services follow the published GA4GH OpenAPI layouts for their versions. |
| **Conformance-style checks** | Community-oriented automated checks: **[HelixTest](https://github.com/SynapticFour/HelixTest)** in Ferrum mode (see [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md)); align expectations with official **testbeds** and implementers’ forums rather than assuming CI covers every spec edge case. |

**Not every GA4GH product has a single global registry** you must register with to be “valid”—interop is usually **pairwise** (your client ↔ your Ferrum base URL + auth). For **federated** setups (e.g. Beacon networks), follow that network’s registration and metadata requirements.

---

## DRS (Data Repository Service)

- **Version:** 1.4  
- **Specification:** [ga4gh/data-repository-service-schemas](https://ga4gh.github.io/data-repository-service-schemas/)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/ga4gh/drs/v1/objects/{object_id}` | Get object metadata | Optional (config) |
| GET | `/ga4gh/drs/v1/objects/{object_id}/access/{access_id}` | **JSON** AccessUrl: download **URL** (+ optional headers); **not** object bytes | Yes (when auth enabled) |
| GET | `/ga4gh/drs/v1/objects/{object_id}/stream` | **Binary** octet stream of object bytes; Crypt4GH at-rest objects decrypted server-side when configured | Optional (dataset rules apply) |
| POST | `/ga4gh/drs/v1/objects` | Create object (admin) | Yes |
| PUT | `/ga4gh/drs/v1/objects/{object_id}` | Update object (admin) | Yes |
| DELETE | `/ga4gh/drs/v1/objects/{object_id}` | Delete object (admin) | Yes |
| GET | `/ga4gh/drs/v1/objects` | List objects (paginated) | Optional |
| GET | `/ga4gh/drs/v1/service-info` | Service info | No |
| POST | `/ga4gh/drs/v1/ingest/file` | Multipart file upload | Yes |
| POST | `/ga4gh/drs/v1/ingest/url` | Ingest from URL | Yes |
| POST | `/ga4gh/drs/v1/ingest/batch` | Batch ingest | Yes |

### Versioned ingest API (Lab Kit / automation)

Stable routes on the **gateway** (same auth as DRS). Errors: JSON `{ "code", "message", "details?" }`. Reference: [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md).

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/ingest/register` | Register URLs or existing storage objects (metadata-only bind) |
| POST | `/api/v1/ingest/upload` | Multipart upload; optional Crypt4GH; `client_request_id` for idempotency |
| GET | `/api/v1/ingest/jobs/{job_id}` | Job status and result |

**Ferrum extensions:** Ingest under `/ga4gh/drs/v1/ingest/*` and `/api/v1/ingest/*`; `GET .../stream` for plaintext delivery of Crypt4GH-encrypted blobs (see [CRYPT4GH.md](CRYPT4GH.md)); Crypt4GH header re-wrap via `X-Crypt4GH-Public-Key` where layered.  
**Limitations:** Bundles use same storage model; no custom `access_id` types beyond `https`.

### DRS: `GET .../access/{access_id}` vs `GET .../stream`

| Endpoint | Response | Notes |
|----------|----------|--------|
| **`.../access/{access_id}`** | **`application/json`** — `url` string (and optional `headers`, `expires_at`) | `access_url` in the DB may be a JSON string or `{"url":"…"}`; both are read correctly. For **S3/MinIO** backends, Ferrum may replace `url` with a **presigned** URL when presigning is configured; if presign fails, the stored URL is returned (logged warning). |
| **`.../stream`** | **Raw bytes** (`Content-Type` from object metadata or `application/octet-stream`) | Direct read from storage (with optional Crypt4GH decrypt). **Not** a JSON wrapper. |

Same distinction as [TES-DOCKER-BACKEND.md — DRS: metadata vs bytes](TES-DOCKER-BACKEND.md#drs-metadata-vs-bytes-cross-reference).

### Nested containers & WES → TES volume strategy

Workflow engines started via **WES → TES** may run **nested** containers (`docker`/`podman` inside the task). Host bind mounts and scratch paths must be consistent on the **host** (see [TES-DOCKER-BACKEND.md — Nested container execution / Host path contract](TES-DOCKER-BACKEND.md#nested-container-execution--host-path-contract-wes--tes)). **Symmetric host path vs `/work`-only** alignment between WES defaults and site mounts is **documented as an open design area**, not fully automated in code.

---

## WES (Workflow Execution Service)

- **Version:** 1.1  
- **Specification:** [ga4gh/workflow-execution-service-schemas](https://ga4gh.github.io/workflow-execution-service-schemas/)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| POST | `/ga4gh/wes/v1/runs` | Submit run | Yes |
| GET | `/ga4gh/wes/v1/runs/{run_id}` | Get run status | Yes |
| POST | `/ga4gh/wes/v1/runs/{run_id}/cancel` | Cancel run | Yes |
| GET | `/ga4gh/wes/v1/runs` | List runs | Yes |
| GET | `/ga4gh/wes/v1/runs/{run_id}/logs` | Get run logs | Yes |
| GET | `/ga4gh/wes/v1/runs/{run_id}/logs/stream` | Stream logs (SSE) | Yes |
| GET | `/ga4gh/wes/v1/service-info` | Service info | No |

**Ferrum extensions:** SSE log streaming at `/runs/{run_id}/logs/stream`; DRS URI resolution for workflow inputs; SLURM/LSF backend via `workflow_engine_params`.  
**Limitations:** Engine-specific params depend on executor (Nextflow, CWL, WDL, Snakemake).

---

## TES (Task Execution Service)

- **Version:** 1.1  
- **Specification:** [ga4gh/task-execution-service-schemas](https://ga4gh.github.io/task-execution-service-schemas/)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| POST | `/ga4gh/tes/v1/tasks` | Create task | Yes |
| GET | `/ga4gh/tes/v1/tasks/{id}` | Get task | Yes |
| GET | `/ga4gh/tes/v1/tasks` | List tasks | Yes |
| POST | `/ga4gh/tes/v1/tasks/{id}/cancel` | Cancel task | Yes |
| GET | `/ga4gh/tes/v1/service-info` | Service info | No |

**Ferrum extensions:** HPC backends (SLURM, LSF) for task execution; Docker/Podman executors accept optional **`executors[].entrypoint`** (Docker Engine API semantics) for images with a fixed `ENTRYPOINT`. See [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md).
**Limitations:** Input/output mounting follows TES spec; DRS inputs resolved to URLs or paths by executor.

---

## TRS (Tool Registry Service)

- **Version:** 2.0  
- **Specification:** [ga4gh/tool-registry-service-schemas](https://ga4gh.github.io/tool-registry-service-schemas/)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/ga4gh/trs/v2/tools` | List tools | No |
| GET | `/ga4gh/trs/v2/tools/{id}` | Get tool | No |
| GET | `/ga4gh/trs/v2/tools/{id}/versions` | List versions | No |
| GET | `/ga4gh/trs/v2/tools/{id}/versions/{version_id}` | Get version descriptor | No |
| GET | `/ga4gh/trs/v2/service-info` | Service info | No |

**Ferrum extensions:** None beyond standard.  
**Limitations:** Tool registration is admin/DB-driven; no automatic sync from external registries.

---

## Beacon v2

- **Version:** 2.0  
- **Specification:** [ga4gh-beacon/beacon-v2](https://github.com/ga4gh-beacon/beacon-v2)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/ga4gh/beacon/v2/` | Beacon info | No |
| GET | `/ga4gh/beacon/v2/individuals` | Query individuals | Optional (tiered) |
| GET | `/ga4gh/beacon/v2/biosamples` | Query biosamples | Optional |
| GET | `/ga4gh/beacon/v2/datasets` | List datasets | No |
| GET | `/ga4gh/beacon/v2/g_variants` | Query variants | Optional (tiered) |

**Ferrum extensions:** Three access tiers (public, registered, controlled) configurable per dataset.  
**Limitations:** Schema and filters follow Beacon v2; large deployments may require indexing tuning.

---

## Passports (GA4GH DURI)

- **Version:** 1.0  
- **Specification:** [ga4gh-duri/ga4gh-passport-v1](https://github.com/ga4gh-duri/ga4gh-passport-v1)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/passports/v1/keys` | JWKS for Passport verification | No |
| (internal) | Passport validation on DRS/WES/TES/Beacon | Validate Visa claims | N/A |

Passports are **Bearer tokens** presented to DRS, WES, TES, and Beacon. Ferrum validates the Passport JWT and Visa claims (e.g. dataset access) before authorizing the request.

**Ferrum extensions:** Configurable trusted Passport Brokers and Visa assertion sources.  
**Limitations:** Visa format and policies must match configured expectations.

---

## htsget

- **Version:** 1.3.0 (tickets + service-info; data via DRS stream)  
- **Specification:** [hts-specs htsget](https://samtools.github.io/hts-specs/htsget.html)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/ga4gh/htsget/v1/reads/service-info` | GA4GH service-info + `htsget` capability block (reads) | No |
| GET | `/ga4gh/htsget/v1/variants/service-info` | Same for variants | No |
| GET | `/ga4gh/htsget/v1/reads/{id}` | Reads ticket (`id` = one path segment: DRS id / alias; embed `/` as `%2F`) | Optional (dataset rules, same as DRS) |
| POST | `/ga4gh/htsget/v1/reads/{id}` | Reads ticket (JSON body; no query string) | Optional |
| GET | `/ga4gh/htsget/v1/variants/{id}` | Variants ticket | Optional |
| POST | `/ga4gh/htsget/v1/variants/{id}` | Variants ticket | Optional |

**Ferrum behaviour:** Tickets contain one HTTPS URL: `GET /ga4gh/drs/v1/objects/{id}/stream` on this deployment (prefix from `FERRUM_PUBLIC_BASE_URL`, default `https://{FERRUM_DRS_HOSTNAME}`). Genomic range / `fields` / `tags` are validated per spec where required; the stream is always the **full** object (clients may filter; spec allows a superset). `class=header` returns `InvalidInput` (not supported).  
**Limitations:** No server-side slicing by genomic interval; CRAM/BCF only if the stored object is classified as such (mime/name). Enable with `services.enable_htsget` (default true); requires DRS DB (same as gateway DRS).

---

## Conformance testing

Automated checks against the demo stack use [HelixTest](https://github.com/SynapticFour/HelixTest) in Ferrum mode; see [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) for **which APIs and scenarios are exercised in CI** and how to reproduce locally.

---

## Interoperability

- **Using Ferrum DRS with external WES (e.g. Terra):** Point the external WES at Ferrum’s DRS **HTTP base** (e.g. `https://ferrum.example.com/ga4gh/drs/v1`). Use canonical **`drs://<drs_hostname>/{object_id}`** URIs (see DRS `self_uri` / hostname config) or the WES client’s DRS resolution (e.g. FISS) with that base URL—not the object path inside the `drs://` scheme.
- **Using external DRS with Ferrum WES:** Configure workflow inputs with `drs://` URIs for the external DRS; Ferrum WES resolves them via the standard DRS client interface (GET object, GET access).
- **Federated Beacon:** Ferrum Beacon can participate in federated queries by exposing the Beacon v2 API; aggregators can include your Ferrum instance in their network.

---

## Passport and Visa configuration

Configure trusted Passport Brokers and Visa policies in `config.toml`:

```toml
[auth]
require_auth = true
jwks_url = "https://broker.example.com/jwks"
passport_endpoints = ["https://broker.example.com"]
```

Visa policies (e.g. which Visa assertions grant access to which datasets) are configured per deployment; see [ARCHITECTURE.md](ARCHITECTURE.md) for the authorization flow and [INSTALLATION.md](INSTALLATION.md) for auth options.

---

*[← Documentation index](README.md)*
