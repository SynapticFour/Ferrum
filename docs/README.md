# Ferrum documentation index

Documentation is organized into: **Getting Started**, **Architecture**, **Standards**, and **Operations**.

---

## Getting started

| Document | Description |
|----------|-------------|
| [README.md](../README.md) | Project overview, badges, quick start, features, deployment. |
| [INSTALLATION.md](INSTALLATION.md) | Prerequisites, demo, build from source, production install, Ansible, Helm, config reference, upgrading, troubleshooting. |

---

## Architecture

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System diagram, monorepo design, ferrum-core, **ferrum-storage**, service isolation, data flows, DB schema, async streaming, config system, deployment topologies. |
| [STORAGE-BACKENDS.md](STORAGE-BACKENDS.md) | Object storage: `LocalStorage`, `S3Storage`, `put_file`, optional OpenDAL. |
| [PERFORMANCE.md](../PERFORMANCE.md) | TB-scale options: libdeflate, benchmarks, BAM lazy ingest feature, cross-links. |

---

## Strategy / product scope

| Document | Description |
|----------|-------------|
| [GA4GH-LAB-KIT-SCOPE.md](GA4GH-LAB-KIT-SCOPE.md) | **German:** Ferrum vs. a separate “compliance kit” repo — boundaries, phased roadmap, component matrix, non-goals. Useful for consortia and labs planning deployments. |

---

## Standards

| Document | Description |
|----------|-------------|
| [GA4GH.md](GA4GH.md) | DRS, WES, TES, TRS, Beacon, Passports: versions, endpoints, auth, extensions, interoperability, Passport/Visa config. |
| [CRYPT4GH.md](CRYPT4GH.md) | Crypt4GH transparent encryption: header re-wrapping, security invariants, key exchange, key management, client usage. |
| [HTSGET.md](HTSGET.md) | GA4GH htsget 1.3.0 tickets (reads/variants): ticket URLs, service-info, validation and DRS `/stream` mapping. |

---

## Operations

| Document | Description |
|----------|-------------|
| [PROVENANCE.md](PROVENANCE.md) | Data provenance and lineage: DAG model, when edges are recorded, API endpoints, UI, RO-Crate export, configuration. |
| [WORKFLOWS.md](WORKFLOWS.md) | Submitting Nextflow, CWL, WDL, Snakemake via WES; DRS inputs; HPC execution; live log streaming. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Development setup, testing, PR process, adding a GA4GH service, Rust style. |
| [SECURITY.md](../SECURITY.md) | Supported versions, reporting vulnerabilities, security model, operator considerations. |
| [COMPLIANCE.md](COMPLIANCE.md) | Regulatory compliance: GDPR, BDSG, Gaia-X, NIS2, EHDS, GA4GH |
| [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) | **Conformance & QA:** what HelixTest exercises in CI (WES, TES, DRS, TRS, Beacon, htsget, E2E, auth, Crypt4GH), URL mapping, demo object IDs, local runs, CI jobs. |

---

*[← Back to Ferrum README](../README.md)*
