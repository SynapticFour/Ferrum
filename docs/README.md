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
| [ARCHITECTURE.md](ARCHITECTURE.md) | System diagram, monorepo design, ferrum-core, service isolation, data flows, DB schema, async streaming, config system, deployment topologies. |

---

## Standards

| Document | Description |
|----------|-------------|
| [GA4GH.md](GA4GH.md) | DRS, WES, TES, TRS, Beacon, Passports: versions, endpoints, auth, extensions, interoperability, Passport/Visa config. |
| [CRYPT4GH.md](CRYPT4GH.md) | Crypt4GH transparent encryption: header re-wrapping, security invariants, key exchange, key management, client usage. |

---

## Operations

| Document | Description |
|----------|-------------|
| [PROVENANCE.md](PROVENANCE.md) | Data provenance and lineage: DAG model, when edges are recorded, API endpoints, UI, RO-Crate export, configuration. |
| [WORKFLOWS.md](WORKFLOWS.md) | Submitting Nextflow, CWL, WDL, Snakemake via WES; DRS inputs; HPC execution; live log streaming. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Development setup, testing, PR process, adding a GA4GH service, Rust style. |
| [SECURITY.md](../SECURITY.md) | Supported versions, reporting vulnerabilities, security model, operator considerations. |
| [COMPLIANCE.md](COMPLIANCE.md) | Regulatory compliance: GDPR, BDSG, Gaia-X, NIS2, EHDS, GA4GH |
| [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) | Running HelixTest conformance against Ferrum; CI strategy and URL mapping. |

---

*[← Back to Ferrum README](../README.md)*
