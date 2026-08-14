# Changelog

All notable changes to this project will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Security

- **Fail-closed defaults** — `auth.require_auth` defaults to true; Passport admin and federation admin routes stay gated when auth is off; visa roles use exact match (not `str::contains`); JWT `aud` is honored when configured; CORS does not treat empty origins as `*`.
- **TES Docker volumes** — client `volumes` are rejected unless `FERRUM_TES_ALLOWED_BIND_PREFIXES` (or WES work-dir prefix) allowlists the host path. `..` is refused.
- **WES Slurm** — `workflow_url` and work dirs are POSIX-quoted; newlines/NUL refused. `ferrum_backend=lsf` returns an error (LSF is not implemented).
- **htsget honesty** — region queries (`referenceName`/`start`/`end`/POST `regions`) return 400 instead of a whole-file ticket.
- **ADS** — client `X-ADS-Base-URL` is ignored on DRS; gateway ADS proxy no longer honors a client override. SSRF checks include IPv6 ULA/link-local/mapped addresses and DNS resolution.
- **HelixTest stubs** — TES `hello-tes` output and WES synthetic `trs://` / checksum-shaped outputs require explicit `FERRUM_TES_HELIXTEST_STUB` / `FERRUM_WES_HELIXTEST_STUBS` (demo compose only). CI no longer rewrites HelixTest expected hashes.

### Changed

- **Docs / CI** — README, COMPLIANCE, [OPERATOR-TRUST.md](docs/OPERATOR-TRUST.md), and HelixTest workflow describe demo vs pilot honestly (no AES-256-GCM / TLS-enforced claims; Crypt4GH is ChaCha20-Poly1305; TLS is operator proxy). Workflow renamed to Demo lifecycle. CodeQL on PRs; dependency-review fails the job; Dependabot **disabled**; Helm chart 0.2.0; MinIO image pinned; `rust-toolchain.toml` 1.91.1.
- **Build** — `ferrum-discovery` vendors GA4GH Service Info types (Apache-2.0). Optional `ga4gh-clearinghouse` is a git pin (`ga4gh-infra-v0.1.0`) so default `cargo build` does not need a sibling checkout.
- **Audit** — residency append failures are logged (`append_warn`); provenance `record_derived_from` errors are warned, not dropped.

### Security

- **Auth fail-closed** — invalid/expired/unconfigured Bearer returns 401; empty-secret HS256 JWT fallback removed; Passport visas are verified (JWKS RS256/ES256 or HS256 `jwt_secret`) or dropped. When `require_auth` is on: Passport admin, TES mutating/list/get, federation mutating/proxy, TRS `/internal/register`, WES cost endpoints, and anonymous WES run reads are gated. Cohorts ignore spoofable `x-owner-sub`. Demo/`require_auth=false` is unchanged.
- **Crypt4GH proxy** — object id from `/objects/{id}/stream`; pubkey present but cannot rewrap → 400/403/502 (no ciphertext passthrough); tokio mpsc instead of std mpsc busy-spin.
- **Storage keys** — `validate_object_key` rejects `..` (slashes in `drs/{ulid}` still allowed). Encrypted DRS stream rejects HTTP Range.

### Fixed

- **CI / demo auth** — Africa Conformance and `ferrum demo start --edge` (CLI and gateway) set `FERRUM_AUTH__REQUIRE_AUTH=false` explicitly (NON-PILOT). Ingest honors that env override. Linux clippy `unnecessary_cast` on `statvfs` fields allowed for portable libc types. HelixTest Ferrum modes expect HTTP 400 for htsget POST `regions`.
- **VCF Beacon index** — stream-parse local VCF (plain or gzip, 50 MiB cap) and batch INSERT (256) instead of `read_to_end` + per-row writes.
- **Residency audit queries** — time/requester filters run in SQL; full-table fetch remains only for chain verify.
- **Edge `sync_push`** — multipart uses a file stream; chunked resume reads 256 KiB with seek (not the whole object in RAM).
- **Service registry** — listing cached 60s process-wide; register invalidates. Avoids re-list on every ADS resolve.
- **Provenance graph** — DRS/WES node details loaded with two `ANY($1)` queries instead of one query per node.
- **S3 `put_file`**, SQLite pathogen SQL without `$1::text`, WES `nfl` kind map, in-memory zstd only ≤ 8 MiB, chunk upload appends from spool `TempPath`.
- **htsget region queries** — region parameters are **rejected** (HTTP 400). An earlier unreleased note that tickets silently covered the whole object was reversed; that behaviour was unsafe to present as htsget 1.3 slicing.
- **TES noop outputs** — COMPLETE tasks with no declared outputs expose a downloadable `url` (`/ga4gh/tes/v1/demo/echo-output`, the bytes `hello-tes` plus a newline) so HelixTest L2 checksums do not depend on a stale local file.
- **WES HelixTest scatter/gather** — `trs://test-tool/scatter-gather` stub outputs include `scatter_result` (same pattern as echo / demo-bam-to-vcf).
- **Revocation DB errors treat as revoked**; ADS `X-ADS-Base-URL` SSRF-checked; TES Slurm `--wrap` escaped; Passport key fail → unconfigured router; mutex poison `into_inner`; `/health` NTP/statvfs 15s cache; POSIX I/O pool default 2–8 threads; `/view` 2 MiB cap; bandwidth uses real elapsed ms.

### Changed

- **Tokio** — library crates pin workspace features (`macros`, `rt`, `rt-multi-thread`, `sync`, `time`, `fs`, `io-util`, `net`; WES/TES also `process`) instead of `features = ["full"]`. Gateway, CLI, and security-tests still use `full`.
- **`FerrumError::Internal`** — no longer `#[from] anyhow`; typed errors must be mapped explicitly.
- **Workspace** — `ferrum-discovery` is a workspace member.
- **CI cargo-deny** — clone `ga4gh-infra` and run `cargo deny` on the runner (Docker action could not see sibling path deps).
- **HelixTest pin** — `HELIXTEST_REF` / `HELIXTEST_SHA` set to HelixTest `main` `3472d44…` (htsget POST format-only vs regions; Africa/E2E Level 0). `v0.1.0` remains the last published tag.
- **Public docs hygiene** — removed `docs/internal/` and `docs/portfolio/` from the public tree (ops/pilot/marketing drafts); Metadata Store roadmap lives at [METADATA-STORE-ROADMAP.md](docs/METADATA-STORE-ROADMAP.md).

### Removed

- **`DatabaseKeyStore`** stub (always `Ok(None)` / not implemented).
- **`ferrum-drs` `bam-lazy-ingest`** feature (compiled in CI, never called from ingest).

### Known remaining (audit follow-ups)

Not in this cut; still worth tracking:

- **htsget `index_status=ready`** is metadata only — no `.bai`/`.tbi` is built.
- **Clearinghouse Passport validation** still uses `block_in_place` / `block_on` on the request path.
- **Multipart ingest** can still buffer the whole file before `max_upload_bytes` (chunked spool exists).
- **S3 `append_bytes`** is still get + rewrite (LocalStorage uses `O_APPEND`).
- **Vendored `crypt4gh`** unwraps on decrypt of untrusted ciphertext (`[patch.crates-io]`).
- **TES Docker socket** remains env-gated (`FERRUM_TES_DOCKER_MOUNT_SOCKET`); not mounted by default.
- Hygiene: Postgres/SQLite query duplication; federation `PeerRateLimiter` never evicts; TES `get_task` N+1; CLI i18n and WES `sanitize_work_dir` still `allow(dead_code)`.
- **Bus factor 1** — cannot be fixed in code.


### Added

- **Metadata Store M2** — versioning (`version` / `content_sha256` / history table), `If-Match` concurrency, `GET .../versions`, DRS attach/detach `PUT /api/v1/metadata/objects/{id}/metadata_ref`; see [METADATA-STORE.md](docs/METADATA-STORE.md).
- **Metadata Store (M0/M1, optional)** — `[metadata_store] enabled` / `FERRUM_METADATA_STORE__ENABLED`; `/api/v1/metadata/submissions` PUT/POST/GET/list over existing `metadata_submissions`; ADR-025; [METADATA-STORE.md](docs/METADATA-STORE.md); plan [METADATA-STORE-ROADMAP.md](docs/METADATA-STORE-ROADMAP.md).
- **H5 managed single-tenant note** — customer-runbook: one hosted deployment = one tenant (no shared multi-tenant DRS schema in H5).
- **H4 Kenya Edge links** — FIELD-REGULATORY / FIELD-SYNC-QUEUE / FIELD-AUTH-OFFLINE point at Solum `kenya-dpa` offline sync policy (PROVISIONAL; fail-closed transfer).
- **H3 subject bridge constants** — `SOLUM_SUBJECT_METADATA_KEY` / `SOLUM_PURPOSE_METADATA_KEY` locked to Solum ADR 0003; runbook notes Patient.id auto subject-link.
- **H2.1 Teeth** — optional Solum sidecar consent checks (`[solum]` / `FERRUM_SOLUM__*`): bound DRS byte access and WES `POST /runs` fail-closed unless `GET /v1/consent/status` returns `granted`; see [customer-runbook.md](docs/customer-runbook.md).
- **H2 WES fail-closed** — when `FERRUM_AUTH__REQUIRE_AUTH` is enabled, anonymous WES list/submit/cancel/resume/status/log/tasks return **401**; `require_auth_enabled()` helper; pilot compose `FERRUM_AUTH__ISSUER` aligned to public broker URL; customer-runbook notes on visas + issuer mismatch.
- **Phase B guided-pilot pack** — `deploy/configs/pilot.toml` (`require_auth=true`); auth/compute honesty in [customer-runbook.md](docs/customer-runbook.md); intended sibling tags documented in `VERSIONS.lock` (`GA4GH_INFRA_TAG` / `HELIXTEST_TAG`).
- **Pilot demo stack (P3–P9, P5)** — `make up-tes`, `make seed-pilot`, `make smoke-pilot`; TinyGermlineHC smoke (Linux CI hard-fails; Mac soft-warns); cohort/analysis wizard auto-fill for shared ref+truth; WES run `tags` and UI links to cohort/sample; Data Browser workspace filter and seed hints; Crypt4GH demo wiring (`crypt4gh-keys` volume, `ferrum-node-keygen`, encrypt round-trip in `test-tes` / `smoke-pilot`); `seed-pilot-remote.sh` for Fly enrichment.
- **CI** — Docker TES + pilot smoke job; crates.io flake retries and `rust-deps` cache; init musl keygen fix for Alpine.

### Changed

- **ECOSYSTEM Demo cross-link** — document `FERUM_SRC` / `FERRUM_SRC` alias and Demo `make smoke-evidence` / COVERAGE.
- **Docs / README** — Demo defaults called out as NON-PILOT (`require_auth=false`, TES noop); `HELIXTEST_SKIP_AUTH` documented as CI convenience; first-release checklist Phase B updates.
- **UI** — Context-aware Crypt4GH upload hints (local keys missing vs hosted pilot); guided empty-state hints on Workflows and Dashboard.
- **Phase 7 — Ecosystem alignment (T7)** — [FIELD-ECOSYSTEM.md](docs/FIELD-ECOSYSTEM.md), [FIELD-GA4GH-DEMO-PI.md](docs/FIELD-GA4GH-DEMO-PI.md), FIELD ecosystem docs; CLI field i18n strings; `ci-field-ecosystem-e2e.sh`; ADR-024.
- **Phase 6 — Operations & resilience (T6)** — `ferrum backup create|restore|verify`; `[ops] verify_checksums_on_startup`; systemd unit + logrotate; [FIELD-OPS.md](docs/FIELD-OPS.md), [FIELD-REGULATORY.md](docs/FIELD-REGULATORY.md); ADR-023; ARM64 50 MB hard gate; `ci-field-ops-e2e.sh`.
- **Phase 5 — Analysis pipeline (T5)** — `[pipeline]` config; post-ingest htsget + Beacon VCF hooks; `ferrum pipeline qc|index-beacon|htsget-status|forward-wes`; `ferrum reference install-field-bundle`; [FIELD-ONT-BASECALLING.md](docs/FIELD-ONT-BASECALLING.md), [FIELD-BEACON-INDEX.md](docs/FIELD-BEACON-INDEX.md); ADR-022; `ci-field-pipeline-e2e.sh`.
- **Phase 4 — Sync & federation (T4)** — `sync_queue` table; `ferrum sync status|enqueue|push|export`; hub multipart push + chunked resume; DUO/consent enqueue filter; sneakernet export; `/api/v1/sync/*`; ga4gh-infra registration on push; [FIELD-SYNC-HUB.md](docs/FIELD-SYNC-HUB.md); ADR-021; `ci-field-sync-e2e.sh`.
- **Phase 3 — Auth & long offline (T2 hardened)** — 7-day JWKS cache + `jwks_file` offline validation; field roles (`ferrum:collector`, `ferrum:analyst`, `ferrum:sync_operator`); `edge_operator_accounts` + `ferrum auth account/login`; JWKS in update bundles; clock skew on `/health`; installer CI smoke; [FIELD-AUTH-OFFLINE.md](docs/FIELD-AUTH-OFFLINE.md); ADR-020.
- **Phase 2 — Metadata & provenance (T3)** — `metadata_ref` on `drs_objects`; `metadata_submissions` table; ferrum-meta validation at ingest (`ferrum_meta` on register/ONT); pathogen + H3Africa profiles in `ferrum-meta-connect`; `ferrum meta init|import`; provenance on ONT ingest (`collection_recorded` residency audit); `ferrum ingest watch --meta-bundle`; schema sync script `scripts/sync-ferrum-meta-schemas.sh`.

## [0.2.0] — 2026-06-11 — Africa resilience release

### Added

- **GISAID metadata at ingest** — optional `gisaid_metadata` on `/api/v1/ingest/register`; stored on `drs_objects.gisaid_metadata`; outbreak activation returns `gisaid_warnings` when `gisaid_auto_package` objects lack required fields; `ferrum outbreak package` reads stored metadata.
- **CLI localisation** — `FERRUM_LANG=fr|de` for Ferrum CLI user-facing strings; documented in [AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md).
- **RO-Crate completeness** — exports include `ont_metrics`, `pathogen_annotations`, and WES `reference_genome` when present.
- **Beacon v2 `$schema`** — all Beacon responses include `meta.$schema`; `/info` and `/service-info` advertise `PathoGenFilter` in `filteringTerms`.
- **Email operations** — [docs/OPERATIONS.md](docs/OPERATIONS.md) and `scripts/check_email_auth.sh` for SPF/DKIM checks on `synapticfour.com`.
- **ARM64 CI** — `build-arm64` job compiles `aarch64-unknown-linux-gnu` release build (Raspberry Pi / ARM server smoke check).
- **Demo start hardening** — 30s production service wait, Laptop Mode fallback, `--force-production` flag.
- **Table security** — PostgreSQL RLS on governance tables; residency audit HTTP filtering for non-admin callers; [COMPLIANCE.md](docs/COMPLIANCE.md) sensitive-table section.
- **`ferrum-reference` crate** — Pluggable reference genome registry (`reference_genomes` table); `/api/v1/references` HTTP API; WES `REFERENCE_MISMATCH` warnings; Beacon `meta.referenceGenome`.
- **HelixTest Africa mode** — `--mode ferrum-africa --africa-profile {offline,ont,outbreak,federation,all}` (opt-in; standard `--mode ferrum` unchanged).
- **Docs** — [REFERENCE-GENOMES.md](docs/REFERENCE-GENOMES.md); Africa Mode in [HELIXTEST-INTEGRATION.md](docs/HELIXTEST-INTEGRATION.md); reference genomes section in [AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md); ADR-016 in [DECISIONS.md](DECISIONS.md).
- **CI** — `.github/workflows/africa-conformance.yml` (additive Africa HelixTest profiles).
- **`ferrum-federation` crate** — P2P federated Beacon fan-out (`FerrumPeer`, `FederationConfig`, rate limits); `GET /ga4gh/beacon/v2/g_variants?federate=true`.
- **Bandwidth-adaptive DRS** — `BandwidthMonitor`, `transfer_checkpoints`, `resume_token` on access, optional `Content-Encoding: zstd` on low-bandwidth streams.
- **Solar/battery mode** — `PowerMonitor`, `FerrumPowerMode`, emergency checkpoint at `~/.ferrum/CHECKPOINT`, concurrency limits via gateway middleware.
- **Data residency audit** — Chained `residency_audit` table; `GET /api/v1/audit/residency` and `/verify`; append-only triggers.
- **Docs** — [FEDERATION.md](docs/FEDERATION.md), [DATA-RESIDENCY-AUDIT.md](docs/DATA-RESIDENCY-AUDIT.md); Africa sections in [AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md); ADR-014/015 in [DECISIONS.md](DECISIONS.md).
- **`ferrum-ont` crate** — ONT types (`OntFormat`, `OntIngestRequest`, `OntQualityMetrics`), validation, DRS field mapping; synthetic POD5 stub for tests.
- **`POST /api/v1/ingest/ont`** — Multipart Nanopore ingest (POD5/FAST5/BLOW5/FASTQ); stores `drs_objects.ont_metrics` and `pathogen_annotations`.
- **Multi-pathogen Beacon** — Optional `organism`, `amrGene`, `serotype`, `minQscore` filters; schema [`crates/ferrum-beacon/schemas/pathogen-extension.json`](crates/ferrum-beacon/schemas/pathogen-extension.json); human genomics queries unchanged without pathogen fields.
- **Outbreak Mode** — `[outbreak]` config, `/api/v1/outbreak/{activate,deactivate,approve-download}`, `outbreak_audit` table, CLI `ferrum outbreak package`.
- **WES template** — [`tools/workflows/ont-qc.wdl`](tools/workflows/ont-qc.wdl) (NanoStat/NanoPlot QC workflow stub).
- **Docs** — [OUTBREAK-MODE.md](docs/OUTBREAK-MODE.md); Nanopore / multi-pathogen / outbreak sections in [AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md); ONT section in [INGEST-LAB-KIT.md](docs/INGEST-LAB-KIT.md); ADR-013 in [DECISIONS.md](DECISIONS.md).
- **`ferrum-embed` crate** — Embedded SQLite + local storage backends for offline-first / Laptop Mode deployments (`EmbedMode`, `SqliteStorage`, `PostgresStorage`, `Database` trait).
- **`[africa]` config profile** — `offline_first`, `max_memory_mb`, `sqlite_path`, `objects_path`; `FERRUM_OFFLINE=1` env shortcut.
- **Laptop Mode quickstart** — `ferrum demo start --offline` (CLI + gateway); auto-fallback when PostgreSQL is unavailable.
- **Native laptop binary** — `./scripts/build-laptop-native.sh` (auto OS/CPU, profile `release-laptop`, `--features laptop`); GitHub Releases ship this optimized single binary.
- **Laptop Mode E2E** — `deploy/scripts/ci-laptop-demo-e2e.sh` (CI job `test-laptop-mode`): health, DRS ingest, metadata, `/stream` round-trip via `ferrum demo start --offline`.
- **Local HelixTest** — `deploy/scripts/run-helixtest-local.sh` (Docker stack + core or `--full` suite).
- **Startup hardening** — Auth endpoint probes wrapped in 5s timeout; non-fatal in offline-first mode.
- **Memory cap** — Optional `africa.max_memory_mb` with Linux VmRSS monitoring.
- **Docs** — [docs/AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md); deployment README Laptop Mode section.
- **CI** — `test-laptop-mode` job (`cargo test -p ferrum-embed` with network namespace isolation where available).
- **`install.sh --offline`** — Install from local build or offline bundle without GitHub/network calls.

### Fixed

- **Demo init / partial DB volumes** — `deploy/scripts/init-demo.sh` records applied migrations in `_ferrum_init_migrations`, bootstraps from `_sqlx_migrations` or existing schema, and skips destructive re-apply on re-init; only pending `.up.sql` files run. Documented in [HELIXTEST-INTEGRATION.md](docs/HELIXTEST-INTEGRATION.md).
- **CI** — `cargo fmt` drift; laptop-mode job falls back when `unshare --net` is unavailable on GitHub runners; E2E script invoked with `bash`; Clippy fixes across workspace.
- **ONT ingest API** — response includes `drs_object_id` alias alongside `object_id` for HelixTest Africa ONT profile.
- **DRS GET** — `ont_metrics` included on object metadata when present (laptop/SQLite and Postgres).
- **WES → TES** — Cancel uses **`POST …/tasks/{id}/cancel`** (Ferrum TES route), not `…:cancel`.
- **DRS /stream** — `storage.get` **NotFound** maps to **404** (not opaque 500).
- **Gateway / DRS** — Object storage init merges **`FERRUM_STORAGE__*`** env; **`minio`** backend treated like **`s3`**; DRS router registers **`/stream`** before **`/objects/:object_id`**.

### Changed

- **Docs** — [docs/README.md](docs/README.md): licensing/compliance; [docs/GA4GH.md](docs/GA4GH.md): GA4GH interoperability guidance.

## [0.1.0] and earlier

### Added

- **MII Connect** — `ferrum mii sync-manifest` regenerates `profiles/mii/manifest.json` from pinned FHIR NPM packages (`profiles/mii/sync-spec.json`), with optional cache under `profiles/mii/package-cache/` and `package_sha256` per package. Library support in `ferrum-mii-connect::sync` (StructureDefinition extraction from `.tgz`). Docs: [docs/MII-CONNECT.md](docs/MII-CONNECT.md), [SECURITY.md](SECURITY.md) (supply-chain note).
- **TES Docker (Bollard)** — `CreateTaskRequest.volumes[]` → **`HostConfig.Binds`**; optional **`FERRUM_TES_DOCKER_*`** env (`MOUNT_SOCKET`, `CLI_HOST_PATH` + `CLI_CONTAINER_PATH`, `NETWORK_MODE`, `EXTRA_HOSTS`, `PLATFORM`). **`deploy/Dockerfile.gateway`**: build-arg **`FERRUM_GATEWAY_FEATURES`** (e.g. `tes-docker`).
- **Gateway** — Cargo feature **`tes-docker`** → enables **`ferrum-tes/docker`** for daemon-backed TES without changing default builds.
- **WES → TES (opt-in)** — Env **`FERRUM_WES_TES_WORK_HOST_PREFIX`**, **`FERRUM_WES_TES_CONTAINER_WORKDIR`**, **`FERRUM_WES_TES_WDL_BASH_LAUNCH`**, **`FERRUM_WES_TES_NEXTFLOW_FILE_LAUNCH`**; default task shape **unchanged** when unset. **`FERRUM_WES_WORKFLOW_URL`** in task env for shell modes.
- **DRS /stream observability** — Response header **`X-Ferrum-DRS-Stream-Path`** (`plaintext` | `crypt4gh_decrypt`); structured logs (`target: ferrum_drs::stream`, `drs.stream.started` / `drs.stream.finished`, byte counters). See [docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md).
- **Demo / CI** — Seeded DRS object **`microbench-plain-v1`** (4096 B, deterministic SHA-256, MinIO `s3` backend) from **`deploy/scripts/init-demo.sh`**; **`deploy/scripts/ci-drs-microbench-stream.sh`**; **`GATEWAY_PUBLIC_URL`** for init (`deploy/docker-compose.yml`). Conformance workflow runs the microbench script before HelixTest.
- **Docs** — [docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md), [docs/WES-WORKFLOW-ENGINES.md](docs/WES-WORKFLOW-ENGINES.md); TES long-run / workdir section in [docs/TES-DOCKER-BACKEND.md](docs/TES-DOCKER-BACKEND.md).
- **WES** — Treat **`NFL`** as **Nextflow** (`workflow_type`) alongside `nextflow` / `nxf` (direct, Slurm, and TES paths).
- **DRS** — `jsonb_to_core_access_url_for_listing` in `access_url` (single place for `GET object` access methods); integration test `tests/access_url_get_access_regression.rs`; utoipa descriptions align **`GET .../access`** (JSON, presign fallback) vs **`GET .../stream`** (binary).
- **Docs** — [docs/TES-DOCKER-BACKEND.md](docs/TES-DOCKER-BACKEND.md) / [docs/GA4GH.md](docs/GA4GH.md): “Nested container execution / Host path contract” and **WES→TES** opt-in env (`FERRUM_WES_TES_*`, `FERRUM_TES_DOCKER_*`).
- **Docs** — [docs/BUSINESS-MODEL.md](docs/BUSINESS-MODEL.md): open-core / BUSL guidance, alignment with [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) business model, differentiated commercial paths; cross-links from [docs/COMPLIANCE.md](docs/COMPLIANCE.md) (intro + contact section) and [CONTRIBUTING.md](CONTRIBUTING.md).
- **Tests:** `ferrum-drs` `api_v1` (structured error JSON + register JSON deserialization); `ferrum-core` `IngestConfig::effective_max_upload_bytes`.
- **Web UI:** Data Browser **Upload file** uses `/api/v1/ingest/upload` (optional Crypt4GH); works when UI is served from the gateway and DRS + storage are configured.
- **Lab Kit ingest API (`/api/v1/ingest`)** — `POST …/register` (URL + existing-object registration), `POST …/upload` (multipart, optional Crypt4GH via Ferrum node key), `GET …/jobs/{id}` with **structured JSON errors** (`code`, `message`, optional `details`). Jobs table `drs_ingest_jobs` + optional **`client_request_id`** idempotency. Config: `[ingest]` (`default_encrypt_upload`, optional `max_upload_bytes`). Gateway wires **local object storage** when `storage.backend` is not `s3` (default dir `./ferrum-blobs` or `storage.base_path`). See [docs/INGEST-LAB-KIT.md](docs/INGEST-LAB-KIT.md).
- **`ferrum_crypt4gh::encrypt_bytes_for_pubkey`** — encrypt small blobs for at-rest ingest using the node public key.
- **`ferrum-storage` crate** — `ObjectStorage`, `LocalStorage`, `S3Storage` (moved out of `ferrum-core`). In-memory `put_bytes` uses S3 multipart from **5 MiB** with bounded concurrency; **`S3Storage::put_file`** streams large **on-disk** uploads (**8 MiB** threshold, **64 MiB** parts, parallel parts, abort incomplete multipart on error). Optional **`opendal`** feature: `OpenDalStorage` for many backends (see [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md)).
- **PostgreSQL pool tuning** — `database.min_connections`, `acquire_timeout_secs`, `idle_timeout_secs`, `max_lifetime_secs`; default `max_connections` scales with `available_parallelism` (clamped 10–100). SQLite pools get acquire timeout.
- **DRS streaming backpressure** — Plaintext `GET …/stream` uses a **bounded channel** and read timeout; Crypt4GH path keeps bounded decrypt→HTTP channel with client send timeout.
- **Graceful shutdown** (gateway) — `503` + `Retry-After` for new DRS stream requests during drain; in-flight stream tracking; `FERRUM_DRAIN_TIMEOUT_SECS` (default 300).
- **Optional build features** — `ferrum-core/libdeflate` (re-exports `noodles_bgzf` for faster BGZF; needs system libdeflate); `ferrum-beacon` feature to pull `libdeflate` through core.
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
