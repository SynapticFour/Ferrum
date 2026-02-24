# GA4GH standards implementation

This document lists each GA4GH standard implemented by Ferrum: version, specification link, endpoints, auth requirements, Ferrum-specific extensions, and known limitations.

---

## DRS (Data Repository Service)

- **Version:** 1.4  
- **Specification:** [ga4gh/data-repository-service-schemas](https://ga4gh.github.io/data-repository-service-schemas/)

| Method | Path | Description | Auth required |
|--------|------|-------------|----------------|
| GET | `/ga4gh/drs/v1/objects/{object_id}` | Get object metadata | Optional (config) |
| GET | `/ga4gh/drs/v1/objects/{object_id}/access/{access_id}` | Get access URL or stream | Yes (when auth enabled) |
| POST | `/ga4gh/drs/v1/objects` | Create object (admin) | Yes |
| PUT | `/ga4gh/drs/v1/objects/{object_id}` | Update object (admin) | Yes |
| DELETE | `/ga4gh/drs/v1/objects/{object_id}` | Delete object (admin) | Yes |
| GET | `/ga4gh/drs/v1/objects` | List objects (paginated) | Optional |
| GET | `/ga4gh/drs/v1/service-info` | Service info | No |
| POST | `/ga4gh/drs/v1/ingest/url` | Ingest from URL | Yes |
| POST | `/ga4gh/drs/v1/ingest/batch` | Batch ingest | Yes |

**Ferrum extensions:** Ingest endpoints (`/ingest/url`, `/ingest/batch`), Crypt4GH streaming access (header re-wrap via `X-Crypt4GH-Public-Key`).  
**Limitations:** Bundles use same storage model; no custom `access_id` types beyond `https`.

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

**Ferrum extensions:** HPC backends (SLURM, LSF) for task execution.  
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

## Interoperability

- **Using Ferrum DRS with external WES (e.g. Terra):** Point the external WES at Ferrum’s DRS base URL. Use `drs://ferrum.example.com/ga4gh/drs/v1/objects/{id}` or the WES client’s DRS resolution (e.g. FISS) with Ferrum’s URL.
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
