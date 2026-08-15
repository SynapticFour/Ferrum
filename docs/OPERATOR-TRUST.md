# Operator trust model

This is the institute-facing record of **what Ferrum guarantees**, **what is demo-only**, and **what is not implemented**. It matches the code. Marketing copy and HelixTest CI badges do not override it.

Related: [SECURITY.md](../SECURITY.md), [THREAT_MODEL.md](THREAT_MODEL.md), [customer-runbook.md](customer-runbook.md), [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md), [COMPLIANCE.md](COMPLIANCE.md).

---

## Profiles

| Profile | How to start | Auth | Compute | HelixTest stubs |
|---------|--------------|------|---------|-----------------|
| **Demo (NON-PILOT)** | `make up` / `deploy/docker-compose.yml` / `deploy/configs/local.toml` | `require_auth=false` (must be set explicitly in compose/toml) | TES **noop** | On (`FERRUM_TES_HELIXTEST_STUB`, `FERRUM_WES_HELIXTEST_STUBS`) |
| **Pilot** | `deploy/configs/pilot.toml` + pilot compose overlays | `require_auth=true` | Operator TES/WES | Off |
| **Production** | `deploy/configs/production.toml` / Helm `values-production.yaml` | `require_auth=true` | Operator TES/WES (Docker or Slurm) | Off |
| **Real local TES** | `make up-tes` (`docker-compose.yml` + `docker-compose.tes.yml`) | inherits overlay | Docker TES | Off |

Code default for `[auth] require_auth` is **true**. Demo files override it to false on purpose and must stay labeled NON-PILOT.

---

## Fail-closed behaviour (pilot / production)

Unless a demo flag above is set:

- Unauthenticated data/compute APIs return **401** when `require_auth` is on.
- Passport **admin** and federation **admin** routes require an exact admin visa (`ferrum:admin`), including when demo auth is off.
- Visa roles use **exact** string match, not substring.
- JWT `aud` is checked when `[auth] audience` / `FERRUM_AUTH__AUDIENCE` is set.
- Empty CORS origin lists are **not** treated as `*`.
- Client `X-ADS-Base-URL` is **ignored** (SSRF / credential forwarding). ADS uses the operator-configured client only.
- TES request `volumes` become host binds only if the host path is under `FERRUM_TES_ALLOWED_BIND_PREFIXES` or `FERRUM_WES_TES_WORK_HOST_PREFIX`. Paths with `..` are refused. Unset prefixes → volumes rejected.
- `FERRUM_TES_BACKEND=docker` **does not** fall back to Podman if the daemon is unreachable (process fails closed).
- WES `workflow_engine_params.ferrum_backend=lsf` returns a validation error. **LSF is not implemented.**
- Slurm run scripts POSIX-quote workflow URLs and work dirs; newlines/NUL are refused.
- htsget **rejects** `referenceName` / `start` / `end` / POST `regions` and `class=header` (HTTP 400). Tickets are whole-object DRS streams only.

---

## Explicit demo / HelixTest stubs (NON-PILOT)

| Variable | When true | Default in demo compose | Default in pilot / `up-tes` |
|----------|-----------|-------------------------|-----------------------------|
| `FERRUM_TES_HELIXTEST_STUB` | Noop TES may attach `hello-tes` output URLs; `GET /ga4gh/tes/v1/demo/echo-output` is served | `true` | `false` |
| `FERRUM_WES_HELIXTEST_STUBS` | Synthetic `trs://test-tool/…` dispositions and stub `outputs` JSON | `true` | `false` |
| `HELIXTEST_SKIP_AUTH` | HelixTest skips Auth Level 4 (CI env, not Ferrum runtime) | set in Demo lifecycle workflow | unset for pilot evidence |

The GitHub Actions workflow [`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) is named **Demo lifecycle (HelixTest)**. It is **not** GA4GH certification and **not** institute evidence. CI does **not** rewrite HelixTest expected checksums. Real compute evidence is `make up-tes` / CI job `test-tes`.

---

## Cryptography (honest claims)

| Claim | Reality |
|-------|---------|
| Crypt4GH at rest | ChaCha20-Poly1305 when encryption is **configured**. `[ingest] default_encrypt_upload` defaults to **false**. |
| TLS | Terminated by the **operator** reverse proxy. Ferrum does not “enforce TLS” by itself. |
| AES-256-GCM | **Not** used for genomic objects. Do not cite COMPLIANCE or marketing for AES-256-GCM. |
| JWT | RS256/ES256 via JWKS, or HS256 with `jwt_secret`. Empty-secret HS256 fallback is removed. |

---

## Build and supply chain

- **MSRV / CI Rust:** 1.91.1 (`rust-toolchain.toml`).
- Default `cargo build` / `cargo test` does **not** require a sibling `ga4gh-infra` checkout. `ferrum-discovery` vendors GA4GH Service Info types (Apache-2.0).
- Optional Passport clearinghouse: `--features clearinghouse` (gateway `external-auth`) pulls `ga4gh-clearinghouse` from GitHub tag `ga4gh-infra-v0.2.2`.
- `cargo deny check` in CI (`deny.toml`). RUSTSEC-2023-0071 (`rsa` Marvin) is **ignored until upstream ships a CT fix** — residual accepted for JWT/Passport paths.
- Helm chart `version` / `appVersion`: **0.2.0**. MinIO image is pinned (not `:latest`).
- **Dependabot is disabled** (`.github/dependabot.yml` has no update ecosystems). Bump crates/actions in PRs against MSRV 1.91.1.

---

## Not implemented (do not advertise)

- BAM/VCF **region slicing** (htsget returns 400 for region params).
- htsget `class=header`.
- LSF job submission.
- Teaching-to-the-test HelixTest checksum rewriting as “conformance.”

---

## Operator env reference (security-relevant)

| Variable | Purpose |
|----------|---------|
| `FERRUM_AUTH__REQUIRE_AUTH` | Override config `require_auth` (`true`/`1`/`yes`/`on` vs `false`). Unset → code/config default (true unless demo toml/compose sets false). |
| `FERRUM_AUTH__AUDIENCE` | Expected JWT `aud`. |
| `FERRUM_AUTH__JWKS_URL` / `FERRUM_AUTH__JWKS_FILE` | Token validation. Cache TTL default **3600s** (`FERRUM_AUTH__JWKS_CACHE_TTL_SECS`); field nodes may raise this. |
| `FERRUM_AUTH__ADS_URL` | Operator ADS base (not client headers). |
| `FERRUM_TES_ALLOWED_BIND_PREFIXES` | Comma-separated host path prefixes for TES request volumes. |
| `FERRUM_WES_TES_WORK_HOST_PREFIX` | WES per-run work dir prefix; also treated as an allowed TES bind prefix. |
| `FERRUM_TES_EXTRA_BINDS` | Operator-controlled extra Docker binds (not client TES JSON). |
| `FERRUM_TES_DOCKER_MOUNT_SOCKET` | Opt-in `docker.sock` mount (privileged). |
| `FERRUM_TES_HELIXTEST_STUB` / `FERRUM_WES_HELIXTEST_STUBS` | Demo stubs; must be false/unset in pilots. |
| `FERRUM_DEMO` | Gateway fail-open auth when set with **no** config file. Demo CLI (`ferrum demo start --edge`) also sets this. |
| `FERRUM_AUTH__REQUIRE_AUTH=false` | Explicit NON-PILOT open ingest/APIs. Africa Conformance and `ferrum demo start --edge` (when unset) set this. Code default remains **true**. |

TES Docker nested-engine variables: [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md).

---

## Pilot acceptance (minimum)

1. `require_auth=true`, real JWKS, no HelixTest stub env vars, no `HELIXTEST_SKIP_AUTH`.
2. TES backend is Docker or Slurm as intended — not noop — if compute is in scope.
3. `FERRUM_TES_ALLOWED_BIND_PREFIXES` (or WES work prefix) is a **tight** allowlist, not `/`.
4. TLS at the proxy; Crypt4GH keys if at-rest encryption is required.
5. Threat-model review: [THREAT_MODEL.md](THREAT_MODEL.md). Incident path: [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md).
