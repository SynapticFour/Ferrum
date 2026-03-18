# HelixTest integration with Ferrum

[HelixTest](https://github.com/SynapticFour/HelixTest) is a Rust-based GA4GH conformance and integration test suite. It can run against any GA4GH-style deployment and has a **Ferrum mode** (`--mode ferrum`) and optional **auto-start** (`--start-ferrum` via Docker). This document describes how to run HelixTest against Ferrum and how Ferrum CI uses it.

---

## Why integrate HelixTest?

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2
- **Workflow execution tests** (CWL, WDL, Nextflow via WES)
- **Cross-service E2E** (TRS → DRS → WES → TES → Beacon)
- **Auth** (GA4GH Passports / OIDC) and **Crypt4GH** tests
- **CI-ready**: exit codes, `--report json`, `--fail-level N`

Test results are **informational only** and do not constitute official GA4GH certification (see HelixTest disclaimer).

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
| `AUTH_URL`             | `{base}/passports/v1` (or Keycloak if testing OIDC) |

Example with gateway on `http://localhost:8080`:

```bash
export WES_URL=http://localhost:8080/ga4gh/wes/v1
export TES_URL=http://localhost:8080/ga4gh/tes/v1
export DRS_URL=http://localhost:8080/ga4gh/drs/v1
export TRS_URL=http://localhost:8080/ga4gh/trs/v2
export BEACON_URL=http://localhost:8080/ga4gh/beacon/v2
export AUTH_URL=http://localhost:8080/passports/v1
```

---

## Running HelixTest against Ferrum locally

1. **Start Ferrum** (e.g. demo stack):

   ```bash
   cd /path/to/Ferrum
   make demo
   # or: docker compose -f deploy/docker-compose.yml up -d
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
   Same stack startup, then runs HelixTest with `--only wes --only tes --only drs --only trs --only beacon` so only the core GA4GH API services are included in the run (and in the report). Useful for faster feedback when you care mainly about DRS/WES/TRS/TES/Beacon.

Both jobs clone HelixTest from GitHub. The ref (branch or tag) is set by the `HELIXTEST_REF` env var at the top of the workflow (default: `main`). To pin to a specific version, set it to a tag (e.g. `v0.1.0`) when HelixTest publishes releases.

You can adjust `--fail-level` (e.g. `2`) in the workflow for stricter gating.

---

## Optional: `--start-ferrum`

HelixTest’s CLI can start Ferrum via Docker before running tests (`--start-ferrum`). That assumes HelixTest is run from a context where `docker compose` refers to a Ferrum stack (e.g. a `docker/` directory in HelixTest with a compose file that points at Ferrum images or build). For **Ferrum’s own CI**, we start the stack from the Ferrum repo and then run HelixTest with the URL env vars; we do **not** rely on `--start-ferrum` so that the same Ferrum code and compose file are under test.

---

*[← Documentation index](README.md)*
