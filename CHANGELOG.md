# Changelog

All notable changes to this project will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **DRS /stream** — `storage.get` **NotFound** maps to **404** (not opaque 500). **Init microbench:** `deploy/scripts/init-demo.sh` seeds **`microbench-plain-v1` last** (after DRS/TRS URL seeds), **UPSERT**s Postgres rows (repairs `ON CONFLICT DO NOTHING` partials), re-`mc alias set` + retried `mc cp`/`mc stat`, and **fails init** if `storage_references` count ≠ 1. Conformance **verify** waits on **`GET …/objects/microbench-plain-v1`**; **`ci-drs-microbench-stream.sh`** prints metadata on stream failure.
- **Gateway / DRS** — Object storage init merges **`FERRUM_STORAGE__*`** env into the loaded `StorageConfig` so **`S3_ENDPOINT` / bucket / keys** are never dropped (without MinIO endpoint the AWS SDK targets **real S3** → **`GET …/stream` 404** while metadata **200**). **`minio`** backend treated like **`s3`**. **S3 init errors** are logged (no longer silent). DRS router: **`/stream`** (and other sub-routes) registered **before** **`/objects/:object_id`**. Stream path **trims** `storage_key`.
- **DRS** — `GET .../access/{access_id}` resolves `access_url` stored as JSON **`{"url": "…"}`** (same shape as create/ingest writes), not only a plain JSON string.
- **TES** — Optional **`executors[].entrypoint`** for Docker (Bollard), Podman CLI, and Slurm-wrapped `podman run`; documents shell/ENTRYPOINT pitfalls in **`docs/TES-DOCKER-BACKEND.md`**.
- htsget routing reliability: compose router/state so ticket endpoints don’t 404 with empty bodies (HelixTest htsget suite).
- CI reliability: build the gateway using an official mirror (ECR public) and retry gateway Docker builds when registries are temporarily flaky.

### Changed

- **Docs** — [docs/README.md](docs/README.md): **Licensing, compliance, and disclaimers**. [docs/GA4GH.md](docs/GA4GH.md): **If you already work with GA4GH APIs** (service-info / API base pattern, Passports, `drs://`, OpenAPI, HelixTest); corrected **`drs://` interoperability** example.
- **Docs / legal clarity** — [BUSINESS-MODEL.md](docs/BUSINESS-MODEL.md): LICENSE prevails, jurisdiction, no implied warranty. [COMPLIANCE.md](docs/COMPLIANCE.md): frameworks non-exhaustive. [SECURITY.md](SECURITY.md): Crypt4GH when configured. [CONTRIBUTING.md](CONTRIBUTING.md): employer permission, tests/docs. [README.md](README.md): licensing in legal notice.

### Added

- **DRS /stream observability** — Response header **`X-Ferrum-DRS-Stream-Path`** (`plaintext` | `crypt4gh_decrypt`); structured logs (`target: ferrum_drs::stream`, `drs.stream.started` / `drs.stream.finished`, byte counters). See [docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md).
- **Demo / CI** — Seeded DRS object **`microbench-plain-v1`** (4096 B, deterministic SHA-256, MinIO `s3` backend) from **`deploy/scripts/init-demo.sh`**; **`deploy/scripts/ci-drs-microbench-stream.sh`**; **`GATEWAY_PUBLIC_URL`** for init (`deploy/docker-compose.yml`). Conformance workflow runs the microbench script before HelixTest.
- **Docs** — [docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md), [docs/WES-WORKFLOW-ENGINES.md](docs/WES-WORKFLOW-ENGINES.md); TES long-run / workdir section in [docs/TES-DOCKER-BACKEND.md](docs/TES-DOCKER-BACKEND.md).
- **WES** — Treat **`NFL`** as **Nextflow** (`workflow_type`) alongside `nextflow` / `nxf` (direct, Slurm, and TES paths).
- **DRS** — `jsonb_to_core_access_url_for_listing` in `access_url` (single place for `GET object` access methods); integration test `tests/access_url_get_access_regression.rs`; utoipa descriptions align **`GET .../access`** (JSON, presign fallback) vs **`GET .../stream`** (binary).
- **Docs** — [docs/TES-DOCKER-BACKEND.md](docs/TES-DOCKER-BACKEND.md) / [docs/GA4GH.md](docs/GA4GH.md): “Nested container execution / Host path contract” and **WES→TES volume strategy** (implemented vs documented-only).
- **Docs** — [docs/BUSINESS-MODEL.md](docs/BUSINESS-MODEL.md): open-core / BUSL guidance, alignment with [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) business model, differentiated commercial paths; cross-links from [docs/COMPLIANCE.md](docs/COMPLIANCE.md) (intro + contact section) and [CONTRIBUTING.md](CONTRIBUTING.md).
- **Tests:** `ferrum-drs` `api_v1` (structured error JSON + register JSON deserialization); `ferrum-core` `IngestConfig::effective_max_upload_bytes`.
- **Web UI:** Data Browser **Upload file** uses `/api/v1/ingest/upload` (optional Crypt4GH); works when UI is served from the gateway and DRS + storage are configured.
- **Lab Kit ingest API (`/api/v1/ingest`)** — `POST …/register` (URL + existing-object registration), `POST …/upload` (multipart, optional Crypt4GH via Ferrum node key), `GET …/jobs/{id}` with **structured JSON errors** (`code`, `message`, optional `details`). Jobs table `drs_ingest_jobs` + optional **`client_request_id`** idempotency. Config: `[ingest]` (`default_encrypt_upload`, optional `max_upload_bytes`). Gateway wires **local object storage** when `storage.backend` is not `s3` (default dir `./ferrum-blobs` or `storage.base_path`). See [docs/INGEST-LAB-KIT.md](docs/INGEST-LAB-KIT.md).
- **`ferrum_crypt4gh::encrypt_bytes_for_pubkey`** — encrypt small blobs for at-rest ingest using the node public key.
- **`ferrum-storage` crate** — `ObjectStorage`, `LocalStorage`, `S3Storage` (moved out of `ferrum-core`). In-memory `put_bytes` uses S3 multipart from **5 MiB** with bounded concurrency; **`S3Storage::put_file`** streams large **on-disk** uploads (**8 MiB** threshold, **64 MiB** parts, parallel parts, abort incomplete multipart on error). Optional **`opendal`** feature: `OpenDalStorage` for many backends (see [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md)).
- **PostgreSQL pool tuning** — `database.min_connections`, `acquire_timeout_secs`, `idle_timeout_secs`, `max_lifetime_secs`; default `max_connections` scales with `available_parallelism` (clamped 10–100). SQLite pools get acquire timeout.
- **DRS streaming backpressure** — Plaintext `GET …/stream` uses a **bounded channel** and read timeout; Crypt4GH path keeps bounded decrypt→HTTP channel with client send timeout.
- **Graceful shutdown** (gateway) — `503` + `Retry-After` for new DRS stream requests during drain; in-flight stream tracking; `FERRUM_DRAIN_TIMEOUT_SECS` (default 300).
- **Optional build features** — `ferrum-core/libdeflate` (re-exports `noodles_bgzf` for faster BGZF; needs system libdeflate); `ferrum-drs/bam-lazy-ingest` (`ingest::bam::scan_alignment_start_positions` via `lazy_records()`); `ferrum-beacon` feature to pull `libdeflate` through core.
- **`ferrum-bench`** — Criterion benchmarks (compile with `cargo bench -p ferrum-bench --no-run`); CI job `bench-and-features` compiles benches and optional features.
- **Docs** — [PERFORMANCE.md](PERFORMANCE.md), [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md), [docs/TES-DOCKER-BACKEND.md](docs/TES-DOCKER-BACKEND.md) (TES Docker/Podman, nested `docker run`, DRS access vs stream).
- TB-scale hardening (Lesson 3): dedicated Rayon pool for blocking POSIX filesystem I/O (`ferrum_core::io::posix`, tunable via `FERRUM_POSIX_IO_THREADS`); `LocalStorage` put/delete/exists/size and Crypt4GH `LocalKeyStore` key file reads use it instead of Tokio’s default blocking pool. TES SLURM backend logs a one-time warning when GNU libc &lt; 2.24 (slow `fork`-based process spawn on some clusters).
- Crypt4GH / hot path: **`Bytes`**-based header rewrap and related throughput-oriented refactors (see crate benchmarks).
- Initial implementation of all GA4GH services (DRS, WES, TES, TRS, Beacon v2, Passports).
- Transparent Crypt4GH encryption layer with header re-wrapping (O(1) per download).
- WES support for Nextflow, CWL, WDL, Snakemake.
- Beacon v2 with three access tiers.
- Single-command demo deployment (Docker Compose, Makefile, init script).
- Helm chart and Ansible playbooks for production and HPC.
- GitHub Actions CI (test, clippy) and release workflows (multi-arch binaries).
- Install script (`install.sh`) for macOS and Linux.
- Documentation: README, ARCHITECTURE, INSTALLATION, CRYPT4GH, GA4GH, WORKFLOWS, CONTRIBUTING, SECURITY, PERFORMANCE, STORAGE-BACKENDS, docs index.
- htsget 1.3.0 ticket/stream integration (tickets refer to DRS `/stream` URLs).

### Changed

- DRS file/batch-path ingest stores **`storage_backend`** from gateway config (`local`, `s3`, …) instead of always `"local"`.
- **`encrypt=true`** on multipart ingest now performs **Crypt4GH encryption** to the node public key before storage (requires `crypt4gh_key_dir` / master key id).

### API stability

- **`/api/v1/ingest/*`** is the supported **versioned** contract for external automation (e.g. Lab Kit). Treat path or response-shape breaks as **semver-major** for consumers relying on this surface.

---

*[← Back to README](README.md)*
