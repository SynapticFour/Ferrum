# HelixTest integration with Ferrum

[HelixTest](https://github.com/SynapticFour/HelixTest) is a Rust-based GA4GH conformance and integration test suite. It can run against any GA4GH-style deployment and has a **Ferrum mode** (`--mode ferrum`) and optional **auto-start** (`--start-ferrum` via Docker). This document describes how to run HelixTest against Ferrum and how Ferrum CI uses it.

---

## Why integrate HelixTest?

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2, **htsget tickets** (whole-object; region slice is not implemented)
- **Workflow execution tests** (CWL, WDL, Nextflow via WES)
- **Cross-service E2E** (TRS → DRS → WES → TES → Beacon)
- **Auth** (GA4GH Passports / OIDC) and **Crypt4GH** tests
- **CI-ready**: exit codes, `--report json`, `--fail-level N`

Test results are **informational only** and do not constitute official GA4GH certification (see HelixTest disclaimer).

---

## What is tested in Ferrum CI? (coverage matrix)

This section is the **single place** in the Ferrum repo that maps **GitHub Actions** to **HelixTest** behaviour. For the authoritative list of individual checks, see the [HelixTest](https://github.com/SynapticFour/HelixTest) sources (e.g. `helixtest/docs/ferrum.md` and the `framework` crate).

### Workflow: [`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml)

**This workflow is a demo-stack lifecycle check, not GA4GH certification or institute evidence.** Demo compose enables HelixTest stubs, TES noop, and `HELIXTEST_SKIP_AUTH`. Real compute is `make up-tes` / CI `test-tes`.

| Job | Docker stack | HelixTest command (simplified) | Purpose |
|-----|----------------|--------------------------------|---------|
| **HelixTest (full)** | `deploy/docker-compose.yml` (demo + init + gateway) | `ci-drs-microbench-stream.sh` then `cargo run --bin helixtest --release -- --all --mode ferrum --report json --fail-level 1` | Demo API lifecycle; report uploaded as artifact. |
| **HelixTest (core services)** | Same stack | Per-service `--only` steps | Fast feedback in the Actions UI. |
| **DRS /stream microbench** | Same stack | `deploy/scripts/ci-drs-microbench-stream.sh` (after gateway healthy) | **Fast path** (no HelixTest): plaintext **`microbench-plain-v1`** stream, **4096** bytes, SHA-256 check, **`X-Ferrum-DRS-Stream-Path`**. Catches DRS stream / MinIO seed regressions before heavy suites. |

HelixTest is cloned from GitHub on each run; the ref is **`HELIXTEST_REF`** / **`HELIXTEST_SHA`** in `VERSIONS.lock` (tag **`v0.1.3`**). Auth-heavy Level‑4 behaviour is skipped in demo CI as documented below. CI does **not** rewrite HelixTest expected checksums.

### Workflow: [`.github/workflows/helixtest-ferrum-infra.yml`](../.github/workflows/helixtest-ferrum-infra.yml)

**Passport co-deploy evidence (scheduled 03:45 UTC + `workflow_dispatch`). Not required on every PR. Not GA4GH certification.** Stack: `make up-pilot-local` (Ferrum `[auth] mode = "external"` + ga4gh-infra mock-idp). HelixTest `--mode ferrum+infra --profile ferrum-infra` with `GATEWAY_BASE=http://localhost:8080`. The job **fails** if any infra check skips or fails (broker, registry, DRS registration, broker Passport, Passport-on-DRS). This is the public identity-plane proof — complementary to HMAC `helixtest-pilot-auth.yml`.

Local:

```bash
make up-pilot-local   # needs sibling ../ga4gh-infra
# Ferrum chmod 644 on cloned PEMs: tagged ga4gh-infra-v0.2.3 leaves them 600,
# which visa-registry (uid 1000) cannot read.
helixtest --all --mode ferrum+infra --profile ferrum-infra-pilot
```

(`ferrum-infra.toml` is the Demo on port 18080; `ferrum-infra-pilot.toml` matches this Ferrum path on 8080.)

### Workflow: [`.github/workflows/helixtest-pilot-auth.yml`](../.github/workflows/helixtest-pilot-auth.yml)

**Pilot-auth evidence (scheduled 02:30 UTC + `workflow_dispatch`). Not required on every PR. Not GA4GH certification.** Stack: `deploy/docker-compose.yml` + `deploy/docker-compose.pilot-auth-ci.yml` with `deploy/configs/pilot.toml` (`require_auth=true`), HelixTest stubs **off**, **`HELIXTEST_SKIP_AUTH` unset**. HelixTest config: [`deploy/helixtest/pilot-auth.toml`](../deploy/helixtest/pilot-auth.toml) (`token-protected-endpoints` on WES `/runs` and TES `/tasks`). HMAC secret is minted per run (`FERRUM_AUTH__JWT_SECRET` / `TEST_BEARER`). This is **not** ga4gh-infra JWKS (`make up-pilot-local`).

Public DRS objects stay readable without a visa; the default HelixTest HMAC-on-DRS fixture is therefore **not** used here.

Ferrum **htsget** rejects genomic region params (`referenceName` / `start` / `end` / POST `regions`) with HTTP 400. HelixTest Ferrum modes expect that 400 (a silent whole-file ticket is not a pass).

### Areas typically included under `--all --mode ferrum`

Exact test names evolve with HelixTest releases; the following **areas** are what Ferrum expects the full run to touch when using the demo stack:

| Area | Examples of what is exercised |
|------|-------------------------------|
| **DRS** | Object metadata, access/stream patterns, service-info, error handling |
| **WES** | Run submit/status/cancel, synthetic `trs://…` workflows, log endpoints, service-info |
| **TES** | Task lifecycle, cancel, service-info; demo uses noop backend with deterministic output |
| **TRS** | Tools, versions, **GA4GH descriptor path** `…/{type}/descriptor` |
| **Beacon v2** | Info, queries, datasets where applicable |
| **htsget** | Service-info (reads/variants), GET/POST tickets, DRS stream URL in ticket, validation errors (`UnsupportedFormat`, `InvalidInput`, etc.) |
| **E2E / interoperability** | Pipeline such as TRS → DRS → WES → TES → Beacon (as defined in HelixTest Ferrum profile) |
| **Auth / Passports** | Scenarios HelixTest defines for Ferrum mode (demo stack uses patched/skip where conflicting) |
| **Crypt4GH** | Encryption-related checks HelixTest runs against Ferrum when enabled in profile |

HelixTest's default `--mode ferrum` does not exercise `/api/v1/ingest/*`. The opt-in `--mode ferrum-africa` profile does (`POST /api/v1/ingest/ont`; `--africa-profile ont`). Lab Kit ingest (`register` / `upload` / `jobs`) stays Ferrum **unit tests** (`ferrum-drs` `api_v1`, `ferrum-core` `[ingest]`) and `scripts/demo_ingest_lab_kit.sh` — that is not the default Ferrum HelixTest ladder. Keep this paragraph in sync with [Helix `INVENTORY.md` §1 Africa](https://github.com/SynapticFour/Helix/blob/main/INVENTORY.md) (canonical list of what HelixTest actually runs; HelixTest has no in-tree `INVENTORY.md`).

### Default CI limitations (read before quoting results externally)

- **Strict auth on every request** (e.g. DRS with `FERRUM_AUTH__REQUIRE_AUTH=true` without sending Bearer on all calls) is **not** the profile the default workflow optimises for; see **Auth (Level 4)** below.
- **htsget** tickets need **`FERRUM_PUBLIC_BASE_URL`** reachable from the test runner (CI sets `http://localhost:8080`).
- **Seeded object IDs** must match HelixTest expectations (e.g. reads `test-object-1`, variants `demo-sample-vcf`); override with `HTSGET_READS_OBJECT_ID` / `HTSGET_VARIANTS_OBJECT_ID` if your seed differs.

For a **customer-facing summary**, the [root README](../README.md#conformance-helixtest) links here; badges on the README point to the latest workflow runs.

---

## Africa Mode (`--mode ferrum-africa`)

Africa-specific conformance is **opt-in** and does not change `--mode ferrum --all` behaviour.

```bash
helixtest --all --mode ferrum-africa --africa-profile offline
helixtest --all --mode ferrum-africa --africa-profile ont
helixtest --all --mode ferrum-africa --africa-profile outbreak
helixtest --all --mode ferrum-africa --africa-profile federation   # needs FERRUM_AFRICA_PEER_URL
helixtest --all --mode ferrum-africa --africa-profile all
```

| Profile | What it exercises |
|---------|-------------------|
| **offline** | Gateway health, DRS/Beacon service-info, reference registry seeds |
| **ont** | `POST /api/v1/ingest/ont`, `ont_metrics`, pathogen Beacon query |

Ferrum CI also runs a **Pi-class watch-folder chain** (`deploy/scripts/ci-minion-field-chain-e2e.sh`, `make minion-chain`): edge gateway with `max_memory_mb=3072`, three simulated POD5 stubs dropped over ~30s, `ferrum ingest watch`, then the same `--africa-profile ont`. That is not a USB MinION, not MinKNOW, and not a real Raspberry Pi (`lab-kit generate pi` remains the hardware path).
| **outbreak** | Outbreak activate/deactivate, residency audit chain verify |
| **federation** | `federate=true` Beacon query (peer URL via `FERRUM_AFRICA_PEER_URL`) |
| **all** | All profiles in sequence |

Fixtures: `HelixTest/helixtest/fixtures/africa/` (ONT stub, example TOML configs).

Ferrum CI runs Africa conformance via [`.github/workflows/africa-conformance.yml`](../.github/workflows/africa-conformance.yml) (additive; [conformance.yml](../.github/workflows/conformance.yml) unchanged).


**Demo DB init (re-runs / partial volumes):** `ferrum-init` runs [`deploy/scripts/init-demo.sh`](../deploy/scripts/init-demo.sh), which tracks applied migrations in `_ferrum_init_migrations` and skips already-applied `.up.sql` files. Upgrading an existing volume applies only pending migrations; idempotent seed steps still run. Full reset: `docker compose -f deploy/docker-compose.yml down -v`. Dev-only journal wipe: `FERRUM_INIT_RESET_MIGRATIONS=1` (re-applies destructive early migrations — do not use on data you need).

---

### TRS descriptor path (GA4GH OpenAPI)

HelixTest follows the official TRS OpenAPI path:

`GET /ga4gh/trs/v2/tools/{id}/versions/{version_id}/{type}/descriptor`

(e.g. `.../demo-bam-to-vcf/versions/demo-bam-to-vcf-1.0/CWL/descriptor`)

Not `.../descriptor/CWL` — that order is a non-standard alias Ferrum also supports for convenience.

### WES / TES / E2E (HelixTest `framework` expectations)

- **WES** uses synthetic `trs://test-tool/...` URLs only when **`FERRUM_WES_HELIXTEST_STUBS`** is set (demo compose default). Without that flag, Ferrum does not invent run states or output JSON.
- **TES checksum stub**: `GET /ga4gh/tes/v1/demo/echo-output` (`hello-tes` + newline) and noop output URLs exist only when **`FERRUM_TES_HELIXTEST_STUB`** is set. CI **does not** rewrite HelixTest expected hashes.
- **E2E** `result_drs_id` stubbing is likewise gated on `FERRUM_WES_HELIXTEST_STUBS`.
- **WES** stub `trs://` runs (echo, scatter-gather, fail) stay `QUEUED` until a second `/status` *and* 1.5s (`RunManager::synthetic_helixtest_phases`), so a concurrent `GET /runs` list cannot make HelixTest's first poll already `COMPLETE`. Demo-only; not institute evidence.

### Auth (Level 4)

HelixTest’s auth tests call DRS **without** attaching the JWT to every request, while also expecting strict 401/403 when `FERRUM_AUTH__REQUIRE_AUTH=true`. Those goals conflict on a single gateway profile.

To keep CI stable, Ferrum’s conformance workflow sets `HELIXTEST_SKIP_AUTH=true`, which makes HelixTest skip the Auth (Level 4) suite in `--mode ferrum`. This avoids false failures while still running the other (non-auth) conformance suites.

**This skip is a CI convenience only — not customer or pilot evidence.** Demo stacks with `require_auth=false` are **NON-PILOT**.

**Nightly auth-on job:** [`.github/workflows/helixtest-pilot-auth.yml`](../.github/workflows/helixtest-pilot-auth.yml) starts the stack with `pilot.toml` (`require_auth=true`), stubs off, `HELIXTEST_SKIP_AUTH` unset, and runs HelixTest `--only auth --fail-level 4` against TES/WES token-protected endpoints. See [customer-runbook.md](./customer-runbook.md) and [OPERATOR-TRUST.md](OPERATOR-TRUST.md).

---

## URL layout: single gateway

Ferrum exposes all GA4GH services behind one **gateway** (default `http://localhost:8080`). HelixTest expects one base URL per service. Map as follows:

| HelixTest env / config | Ferrum endpoint (base = gateway) |
|------------------------|-----------------------------------|
| `WES_URL`              | `{base}/ga4gh/wes/v1`             |
| `TES_URL`              | `{base}/ga4gh/tes/v1`             |
| `DRS_URL`              | `{base}/ga4gh/drs/v1`             |
| `TRS_URL`              | `{base}/ga4gh/trs/v2`             |
| `BEACON_URL`           | `{base}/ga4gh/beacon/v2`          |
| `HTSGET_URL`           | `{base}/ga4gh/htsget/v1`         |
| `AUTH_URL`             | `{base}/passports/v1` (or Keycloak if testing OIDC). Nightly auth-on uses WES `{base}/ga4gh/wes/v1` because Passports have OIDC discovery, not GA4GH `/service-info`. |

**htsget defaults (HelixTest [ferrum.md](https://github.com/SynapticFour/HelixTest/blob/main/helixtest/docs/ferrum.md)):** With gateway-style `WES_URL` / `DRS_URL` (path `/ga4gh/…`), the suite resolves htsget automatically — **`HTSGET_URL` is optional**. Explicit override: `HTSGET_URL` or `GATEWAY_BASE`.

**Demo object IDs (Ferrum `deploy/scripts/init-demo.sh`):**

| Env | Default | Role |
|-----|---------|------|
| `HTSGET_READS_OBJECT_ID` (or legacy `HTSGET_READS_ID`) | `test-object-1` | Reads/BAM-class DRS object (`mime_type` `application/vnd.ga4gh.bam`; storage still URL-backed for DRS access) |
| `HTSGET_VARIANTS_OBJECT_ID` | `demo-sample-vcf` | Variants/VCF (`text/vcf`) |

**Dataset-gated htsget (optional):** When `FERRUM_AUTH__REQUIRE_AUTH=true` and the object has `dataset_id`, set **`HELIXTEST_HTSGET_DATASET_OBJECT_ID`** — expect **403** `PermissionDenied` without `Authorization`. For a **200** ticket, set **`HELIXTEST_HTSGET_DATASET_BEARER`** to a GA4GH Passport / JWT with **ControlledAccessGrants** for that dataset (see HelixTest `framework/src/htsget.rs`).

**Ferrum env for correct ticket links:** Set **`FERRUM_PUBLIC_BASE_URL`** to the URL clients use to reach the gateway (e.g. `http://localhost:8080` in CI). Default: `https://{FERRUM_DRS_HOSTNAME}`. Ferrum CI sets this for [conformance.yml](../.github/workflows/conformance.yml).

Example with gateway on `http://localhost:8080`:

```bash
export WES_URL=http://localhost:8080/ga4gh/wes/v1
export TES_URL=http://localhost:8080/ga4gh/tes/v1
export DRS_URL=http://localhost:8080/ga4gh/drs/v1
export TRS_URL=http://localhost:8080/ga4gh/trs/v2
export BEACON_URL=http://localhost:8080/ga4gh/beacon/v2
export AUTH_URL=http://localhost:8080/passports/v1
export HTSGET_URL=http://localhost:8080/ga4gh/htsget/v1   # optional if WES_URL/DRS_URL use /ga4gh/…
export FERRUM_PUBLIC_BASE_URL=http://localhost:8080       # htsget ticket → DRS /stream URLs
```

---

## Running HelixTest against Ferrum locally

1. **Start Ferrum** (e.g. demo stack):

   ```bash
   cd /path/to/Ferrum
   make demo
   # or: docker compose -f deploy/docker-compose.yml up -d
   # or (one command, core + htsget): ./deploy/scripts/run-helixtest-local.sh
   # full suite: ./deploy/scripts/run-helixtest-local.sh --full
   ```

2. **Clone and run HelixTest** (from a separate directory):

   ```bash
   git clone https://github.com/SynapticFour/HelixTest.git
   cd HelixTest
   export WES_URL=http://localhost:8080/ga4gh/wes/v1
   export TES_URL=http://localhost:8080/ga4gh/tes/v1
   export DRS_URL=http://localhost:8080/ga4gh/drs/v1
   export TRS_URL=http://localhost:8080/ga4gh/trs/v2
   export BEACON_URL=http://localhost:8080/ga4gh/beacon/v2
   export AUTH_URL=http://localhost:8080/passports/v1
   export FERRUM_PUBLIC_BASE_URL=http://localhost:8080
   cargo run --bin helixtest --release -- --all --mode ferrum --report table
   ```

   If the UI is on port 8082 (nginx), the **gateway** is still on 8080; use 8080 for the URLs above.

3. **Optional**: use a TOML config file (see HelixTest `helixtest/README.md`) and set `HELIXTEST_CONFIG` to point to it.

---

## CI strategy (Ferrum repo)

Ferrum’s CI runs HelixTest **on every push and pull request** to `main`/`master`. The workflow [../.github/workflows/conformance.yml](../.github/workflows/conformance.yml) has two jobs:

1. **HelixTest (full)**
   Starts the demo stack, runs `helixtest --all --mode ferrum --report json --fail-level 1`, uploads the JSON report as an artifact, and fails the job if the suite or level check fails.

2. **HelixTest (core services)**
   Same stack startup, then:
   - `Run HelixTest (WES, TES, DRS, TRS, Beacon)` — `--only wes --only tes --only drs --only trs --only beacon`
   - **`Run HelixTest (htsget only)`** — `--only htsget` (separate Actions step for isolated failures)
   htsget is also run in job 1 via `--all` (service-info, GET/POST tickets, DRS stream path in `urls[0]`, error codes, optional dataset-auth via env).

Both jobs clone HelixTest from GitHub. The ref is **`HELIXTEST_REF`** in `VERSIONS.lock` (tag **`v0.1.3`**; SHA in `HELIXTEST_SHA`).

You can adjust `--fail-level` (e.g. `2`) in the workflow for stricter gating.

---

## Optional: `--start-ferrum`

HelixTest’s CLI can start Ferrum via Docker before running tests (`--start-ferrum`). That assumes HelixTest is run from a context where `docker compose` refers to a Ferrum stack (e.g. a `docker/` directory in HelixTest with a compose file that points at Ferrum images or build). For **Ferrum’s own CI**, we start the stack from the Ferrum repo and then run HelixTest with the URL env vars; we do **not** rely on `--start-ferrum` so that the same Ferrum code and compose file are under test.

---

*[← Documentation index](README.md)*
