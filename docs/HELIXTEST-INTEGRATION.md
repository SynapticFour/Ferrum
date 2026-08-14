# HelixTest integration with Ferrum

[HelixTest](https://github.com/SynapticFour/HelixTest) is a Rust-based GA4GH conformance and integration test suite. It can run against any GA4GH-style deployment and has a **Ferrum mode** (`--mode ferrum`) and optional **auto-start** (`--start-ferrum` via Docker). This document describes how to run HelixTest against Ferrum and how Ferrum CI uses it.

---

## Why integrate HelixTest?

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2, **htsget 1.3.0**
- **Workflow execution tests** (CWL, WDL, Nextflow via WES)
- **Cross-service E2E** (TRS → DRS → WES → TES → Beacon)
- **Auth** (GA4GH Passports / OIDC) and **Crypt4GH** tests
- **CI-ready**: exit codes, `--report json`, `--fail-level N`

Test results are **informational only** and do not constitute official GA4GH certification (see HelixTest disclaimer).

---

## What is tested in Ferrum CI? (coverage matrix)

This section is the **single place** in the Ferrum repo that maps **GitHub Actions** to **HelixTest** behaviour. For the authoritative list of individual checks, see the [HelixTest](https://github.com/SynapticFour/HelixTest) sources (e.g. `helixtest/docs/ferrum.md` and the `framework` crate).

### Workflow: [`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml)

| Job | Docker stack | HelixTest command (simplified) | Purpose |
|-----|----------------|--------------------------------|---------|
| **HelixTest (full)** | `deploy/docker-compose.yml` (demo + init + gateway) | `ci-drs-microbench-stream.sh` then `cargo run --bin helixtest --release -- --all --mode ferrum --report json --fail-level 1` | **Microbench** first (fast DRS `/stream` regression), then one gate with **maximum** HelixTest coverage; report uploaded as workflow artifact. |
| **HelixTest (core services)** | Same stack | Step 1: `… --only wes --only tes --only drs --only trs --only beacon --fail-level 2` | Fast feedback in the Actions UI for the “core” GA4GH APIs. |
| | | Step 2: `… --only htsget --fail-level 2` | **htsget** isolated so ticket/stream regressions do not hide inside a large step. |
| **DRS /stream microbench** | Same stack | `deploy/scripts/ci-drs-microbench-stream.sh` (after gateway healthy) | **Fast path** (no HelixTest): plaintext **`microbench-plain-v1`** stream, **4096** bytes, SHA-256 check, **`X-Ferrum-DRS-Stream-Path`**. Catches DRS stream / MinIO seed regressions before heavy suites. |

HelixTest is cloned from GitHub on each run; the ref is **`HELIXTEST_REF`** in the workflow (default `main`). Patch steps align expected checksums with Ferrum’s noop TES backend and seeded DRS URLs; auth-heavy Level‑4 behaviour is skipped in CI as documented below.

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

**Not in HelixTest today:** **`/api/v1/ingest/*`** (Lab Kit ingest) is covered by Ferrum **unit tests** (`ferrum-drs` `api_v1` request/JSON shape, `ferrum-core` `[ingest]` config) and manual/`scripts/demo_ingest_lab_kit.sh` checks — add HelixTest scenarios when the Ferrum profile grows.

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

- **WES** uses synthetic `trs://test-tool/...` URLs (echo, fail, scatter-gather, cwl-echo, `trs://nonexistent/invalid/0.0`). Ferrum maps these to the expected states and TES-backed echo/E2E/`scatter_result` outputs without real workflow engines.
- **TES checksum**: HelixTest compares output bytes (TES `outputs[].url`, else a *fresh* local `tes_echo_out.txt`) to `test-data/expected/`. Demo TES is **noop**; it completes immediately and serves those bytes at `GET /ga4gh/tes/v1/demo/echo-output`. CI still recomputes the expected hash from HelixTest’s committed `tes_echo_out.txt` so the golden matches.
- **E2E** is DRS-first by default: HelixTest submits WES runs with `workflow_params.input_drs_uri` and prefers `drs://...` references for the input object. It expects `outputs.result_drs_id` after a `demo-bam-to-vcf` TRS run; Ferrum sets that to seeded `demo-sample-vcf`. The object’s `access_url` must be an HTTPS URL that returns 200 in CI (init seed uses a stable `raw.githubusercontent.com` file, not EBI FTP). HelixTest’s `expected/e2e/result.txt.sha256` may still contain a placeholder; **CI** runs `deploy/scripts/align-helixtest-e2e-checksum.sh` so the expected hash matches that URL (keep in sync with `init-demo.sh` for `demo-sample-vcf`).
- **WES** negative `trs://` cases must not report a terminal state on the **first** `GET .../runs/{id}/status` poll; Ferrum defers `EXECUTOR_ERROR` to the second poll (see `RunManager::synthetic_helixtest_error_phases`).

### Auth (Level 4)

HelixTest’s auth tests call DRS **without** attaching the JWT to every request, while also expecting strict 401/403 when `FERRUM_AUTH__REQUIRE_AUTH=true`. Those goals conflict on a single gateway profile.

To keep CI stable, Ferrum’s conformance workflow sets `HELIXTEST_SKIP_AUTH=true`, which makes HelixTest skip the Auth (Level 4) suite in `--mode ferrum`. This avoids false failures while still running the other (non-auth) conformance suites.

**This skip is a CI convenience only — not customer or pilot evidence.** Demo stacks with `require_auth=false` are **NON-PILOT**. For pilot auth verification, use `deploy/configs/pilot.toml` (`require_auth=true`), unset `HELIXTEST_SKIP_AUTH`, and prefer a scheduled/nightly auth-on job over enabling Auth Level 4 on every PR. See [customer-runbook.md](./customer-runbook.md).

If you want to validate strict Auth Level 4 end-to-end, unset `HELIXTEST_SKIP_AUTH` and ensure all relevant requests include `Authorization: Bearer ...` (or implement path-specific gateway auth gating).

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
| `AUTH_URL`             | `{base}/passports/v1` (or Keycloak if testing OIDC) |

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

Both jobs clone HelixTest from GitHub. The ref is **`HELIXTEST_REF`** in `VERSIONS.lock` (a commit SHA of HelixTest `main` as of 2026-08-14; last published tag is `v0.1.0`).

You can adjust `--fail-level` (e.g. `2`) in the workflow for stricter gating.

---

## Optional: `--start-ferrum`

HelixTest’s CLI can start Ferrum via Docker before running tests (`--start-ferrum`). That assumes HelixTest is run from a context where `docker compose` refers to a Ferrum stack (e.g. a `docker/` directory in HelixTest with a compose file that points at Ferrum images or build). For **Ferrum’s own CI**, we start the stack from the Ferrum repo and then run HelixTest with the URL env vars; we do **not** rely on `--start-ferrum` so that the same Ferrum code and compose file are under test.

---

*[← Documentation index](README.md)*
