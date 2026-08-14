# Ferrum documentation index

Documentation is organized into: **Getting Started**, **Architecture**, **Standards**, **Field Edge**, and **Operations**.

---

## Licensing, compliance, and disclaimers

| Topic | Where to read |
|--------|----------------|
| **Software license (BUSL-1.1)** — research vs commercial, Change Date → Apache-2.0 | [LICENSE](../LICENSE), [BUSINESS-MODEL.md](BUSINESS-MODEL.md) |
| **API / release compatibility & deprecation** | [COMPATIBILITY.md](COMPATIBILITY.md) |
| **Open core, Lab Kit, keys, enterprise** (high-level only; not a substitute for counsel) | [BUSINESS-MODEL.md](BUSINESS-MODEL.md) |
| **Data protection / sector regulation** (GDPR, EU examples, operator duties) | [COMPLIANCE.md](COMPLIANCE.md) |
| **Operator trust (demo vs pilot, stubs, honest feature matrix)** | [OPERATOR-TRUST.md](OPERATOR-TRUST.md) |
| **Security model, reporting vulnerabilities** | [SECURITY.md](../SECURITY.md) |

**Important:** Docs in this tree are **technical orientation** unless marked otherwise. They are **not legal advice**. Operators and organisations remain responsible for their own compliance, contracts, and jurisdictional requirements.

---

## Getting started

| Document | Description |
|----------|-------------|
| [README.md](../README.md) | Project overview, badges, quick start, features, deployment. |
| [INSTALLATION.md](INSTALLATION.md) | Prerequisites, demo, build from source, production install, Ansible, Helm, config reference, upgrading, troubleshooting. |
| [ECOSYSTEM.md](ECOSYSTEM.md) | **Five-repo SynapticFour stack** — Ferrum, ga4gh-infra, Lab Kit, Demo, HelixTest |
| [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md) | External auth: broker, Passports, clearinghouse, service registry |
| [deployment/README.md](deployment/README.md) | Deployment paths matrix, update/bugfix delivery strategy, preflight checks. |
| [deployment/OFFLINE-AIRGAP.md](deployment/OFFLINE-AIRGAP.md) | Air-gapped deployment flow (export/import bundles). |

---

## Architecture

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System diagram, monorepo design, ferrum-core, **ferrum-storage**, service isolation, data flows, DB schema, async streaming, config system, deployment topologies. |
| [STORAGE-BACKENDS.md](STORAGE-BACKENDS.md) | Object storage: `LocalStorage`, `S3Storage`, `put_file`, optional OpenDAL. |
| [PERFORMANCE.md](../PERFORMANCE.md) | TB-scale options: libdeflate, benchmarks, BAM lazy ingest feature, cross-links. |
| [PERFORMANCE-CRYPT4GH.md](PERFORMANCE-CRYPT4GH.md) | DRS **Plain vs Crypt4GH** benchmarks: comparable objects, curl/Python timing, `X-Ferrum-DRS-Stream-Path`, logs, pitfalls, CI microbench script. |
| [WES-WORKFLOW-ENGINES.md](WES-WORKFLOW-ENGINES.md) | WES **`workflow_type`** matrix (WDL, **Nextflow**, CWL, Snakemake), TES defaults, `workflow_engine_params`, roadmap notes. |
| [FEDERATION.md](FEDERATION.md) | Multi-node federation: registry, peer discovery, federated Beacon |
| [REFERENCE-GENOMES.md](REFERENCE-GENOMES.md) | Reference genome registry and workflow alignment |
| [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md) | Residency audit chains and export |

---

## Product scope

| Document | Description |
|----------|-------------|
| [BUSINESS-MODEL.md](BUSINESS-MODEL.md) | **Open core & BUSL:** research vs commercial use, relation to [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit), differentiated paths (SLA, consortium, delayed Apache-2.0). |
| [GA4GH-LAB-KIT-SCOPE.md](GA4GH-LAB-KIT-SCOPE.md) | Scope boundaries between Ferrum core and Ferrum Lab Kit extensions. |

---

## Standards

| Document | Description |
|----------|-------------|
| [GA4GH.md](GA4GH.md) | DRS, WES, TES, TRS, Beacon, Passports: versions, endpoints, auth, extensions, interoperability, Passport/Visa config. |
| [MII-CONNECT.md](MII-CONNECT.md) | Ferrum MII Connect: technical MII-oriented profile checks, `validate` / `sync-manifest` CLI, config, legal/technical boundaries. |
| [MII-MODULE-MAPPING.md](MII-MODULE-MAPPING.md) | Default-17 module/resource mapping and gap tag taxonomy for MII validation. |
| [MII-CI-INTEGRATION.md](MII-CI-INTEGRATION.md) | CI patterns (GitHub/GitLab), strictness strategy and audit retention for MII reports. |
| [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md) | **Machine ingest for Lab Kit:** `/api/v1/ingest` (register, upload, jobs), auth, Crypt4GH, idempotency, curl examples. |
| [CRYPT4GH.md](CRYPT4GH.md) | Crypt4GH transparent encryption: header re-wrapping, security invariants, and operational key management. |
| [HTSGET.md](HTSGET.md) | GA4GH htsget 1.3.0 tickets (reads/variants): ticket URLs, service-info, validation and DRS `/stream` mapping. |
| [OUTBREAK-MODE.md](OUTBREAK-MODE.md) | Emergency outbreak policies, Beacon/DRS access overrides |

---

## Field Edge (Africa / offline)

| Document | Description |
|----------|-------------|
| [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) | Field Edge deployment guide: hardware, sync, ONT, Beacon |
| [FIELD-OPS.md](FIELD-OPS.md) | Operator runbook: power, bandwidth, updates |
| [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md) | JWKS offline cache, field roles, edge accounts |
| [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md) | Edge → hub sync queue |
| [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md) | Hub-side sync ingestion |
| [FIELD-ONT-BASECALLING.md](FIELD-ONT-BASECALLING.md) | ONT basecalling integration |
| [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md) | VCF → Beacon indexing on Edge |
| [FIELD-ECOSYSTEM.md](FIELD-ECOSYSTEM.md) | Field Edge in the five-repo stack |
| [FIELD-REGULATORY.md](FIELD-REGULATORY.md) | Regulatory context (orientation only) |
| [FIELD-GA4GH-DEMO-PI.md](FIELD-GA4GH-DEMO-PI.md) | Demo/benchmark scenarios for Field Edge |

---

## Operations

| Document | Description |
|----------|-------------|
| [PROVENANCE.md](PROVENANCE.md) | Data provenance and lineage: DAG model, when edges are recorded, API endpoints, UI, RO-Crate export, configuration. |
| [WORKFLOWS.md](WORKFLOWS.md) | Submitting Nextflow, CWL, WDL, Snakemake via WES; DRS inputs; HPC execution; live log streaming. See also [WES-WORKFLOW-ENGINES.md](WES-WORKFLOW-ENGINES.md). |
| [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md) | TES Docker/Podman: **entrypoint vs command**, nested `docker run`, host binds, `docker.sock` vs CLI, WES defaults pointer. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Development setup, testing, PR process, adding a GA4GH service, Rust style. |
| [SECURITY.md](../SECURITY.md) | Supported versions, reporting vulnerabilities, security model, operator considerations. |
| [COMPLIANCE.md](COMPLIANCE.md) | Regulatory compliance: GDPR, BDSG, Gaia-X, NIS2, EHDS, GA4GH |
| [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) | **Demo lifecycle QA:** what HelixTest exercises in CI (not certification). Institute posture: [OPERATOR-TRUST.md](OPERATOR-TRUST.md). |
| [deployment/UPDATE-SOP.md](deployment/UPDATE-SOP.md) | SOP template for controlled updates/bugfixes and rollback. |
| [deployment/RELEASE-CHECKLIST.md](deployment/RELEASE-CHECKLIST.md) | 10 required checks before release/hotfix rollout. |

---

## Internal notes (contributors)


---

*[← Back to Ferrum README](../README.md)*
