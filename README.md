# Ferrum

<!-- logo placeholder -->
<p align="center"><strong>Ferrum</strong></p>

[![CI](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml)
[![Conformance](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/docker/pulls/ferrum/gateway?label=docker%20pulls)](https://hub.docker.com/r/ferrum/gateway)

**Sovereign bioinformatics infrastructure. GA4GH-native. On-premises first. Built in Rust.**

For product scope versus a future **GA4GH compliance starter kit** (orchestration, checklists, audit artefacts around Ferrum), see [`docs/GA4GH-LAB-KIT-SCOPE.md`](docs/GA4GH-LAB-KIT-SCOPE.md).

> **Legal notice:** This README is an overview of technical capabilities, not legal advice.  
> Whether a specific deployment complies with GDPR/DSGVO, NIS2, EHDS, HIPAA or other frameworks depends on the operator’s legal basis, contracts, configuration and organisational measures. Always consult qualified legal counsel for compliance questions.

---

## Why Ferrum?

| Problem | Solution |
|--------|----------|
| **Cloud lock-in** | Runs entirely on your hardware — no vendor SaaS required. |
| **Vendor APIs** | Full [GA4GH](https://www.ga4gh.org/) standard compatibility so you interoperate with the global ecosystem. |
| **Plaintext data** | Zero-plaintext [Crypt4GH](https://www.ga4gh.org/news_item/crypt4gh-encryption-standard/) encryption at rest and per-requester re-encryption on download. |

---

## Features

| | Feature |
|---|--------|
| 🔐 | **Transparent Crypt4GH encryption** — Header re-wrapping; file bodies are designed to avoid re-encryption (O(1) per download). |
| 📦 | **GA4GH stack** — DRS, TRS, WES, TES, Beacon v2, htsget, Passports. |
| ⚡ | **Rust performance** — No GC, predictable latency, minimal footprint. |
| 🔬 | **Workflow engines** — Nextflow, CWL, WDL, Snakemake. |
| 🖥️ | **HPC scheduling** — SLURM and LSF job scheduling. |
| 🚀 | **One-command demo** — `ferrum demo start`; Helm charts for production. |
| 📊 | **Provenance & lineage** — DAG of DRS objects and WES runs; queryable upstream/downstream, visual graph, [RO-Crate](https://w3id.org/ro/crate/1.1) export for citation. |
| GDPR/DSGVO support | Technical features (encryption, provenance, access control) that operators can combine with their own legal and organisational measures. See [COMPLIANCE.md](docs/COMPLIANCE.md). |
| Gaia-X principles | On-premises deployment and GA4GH APIs that can support Gaia-X-style data sovereignty; formal Gaia-X labelling requires separate assessment. |
| EHDS alignment | Uses GA4GH APIs referenced in EHDS discussions; actual EHDS compliance depends on future delegated acts and operator processes. |
| NIS2-related features | Security event log, breach alerting and SBOM tooling that can support NIS2 programmes when properly configured. |

---

## Architecture

```mermaid
flowchart LR
  subgraph Client["Client Tier"]
    UI[Web UI]
    CLI[CLI / SDK]
  end

  subgraph Gateway["Ferrum Gateway"]
    G[Gateway]
  end

  subgraph Services["GA4GH Services"]
    DRS[DRS]
    WES[WES]
    TES[TES]
    TRS[TRS]
    B[Beacon]
    P[Passports]
    HTSGET[htsget]
    C4[Crypt4GH]
  end

  subgraph Backend["Backend"]
    PG[(PostgreSQL)]
    S3[MinIO / S3]
  end

  UI --> G
  CLI --> G
  G --> DRS
  G --> HTSGET
  G --> WES
  G --> TES
  G --> TRS
  G --> B
  G --> P
  HTSGET --> DRS
  G --> C4
  DRS --> C4
  C4 --> PG
  C4 --> S3
  DRS --> PG
  WES --> PG
  TES --> PG
  TRS --> PG
  B --> PG
  P --> PG
```

---

## Quick Start

### 1. Install (macOS / Linux)

```bash
curl -sSf https://raw.githubusercontent.com/SynapticFour/Ferrum/main/install.sh | sh
export PATH="$HOME/.ferrum/bin:$PATH"
```

### 2. Start demo

```bash
ferrum demo start
```

### 3. Use the UI

Open **http://localhost:3000**. The demo includes pre-seeded DRS objects and test users. (Port may vary; run `ferrum status` to confirm.)

---

## GA4GH Standards

| Standard | Version | Status | Endpoint |
|----------|---------|--------|----------|
| [DRS](https://ga4gh.github.io/data-repository-service-schemas/) | 1.4 | ✅ Implemented | `/ga4gh/drs/v1` |
| [WES](https://ga4gh.github.io/workflow-execution-service-schemas/) | 1.1 | ✅ Implemented | `/ga4gh/wes/v1` |
| [TES](https://ga4gh.github.io/task-execution-service-schemas/) | 1.1 | ✅ Implemented | `/ga4gh/tes/v1` |
| [TRS](https://ga4gh.github.io/tool-registry-service-schemas/) | 2.0 | ✅ Implemented | `/ga4gh/trs/v2` |
| [Beacon](https://github.com/ga4gh-beacon/beacon-v2) | 2.0 | ✅ Implemented | `/ga4gh/beacon/v2` |
| [htsget](https://samtools.github.io/hts-specs/htsget.html) | 1.3.0 | ✅ Implemented | `/ga4gh/htsget/v1` |
| [Passports](https://github.com/ga4gh-duri/ga4gh-passport-v1) | 1.0 | ✅ Implemented | `/passports/v1` |
| Crypt4GH | 1.0 | ✅ Implemented | `/ga4gh/crypt4gh/v1` |

---

## Conformance (HelixTest)

Every push and pull request runs the open-source [HelixTest](https://github.com/SynapticFour/HelixTest) suite in **Ferrum mode** against the real demo stack (Postgres, MinIO, Keycloak, seeded data):

| CI job | What runs |
|--------|-----------|
| **HelixTest (full)** | `helixtest --all --mode ferrum` — entire suite HelixTest ships for Ferrum (API contracts, workflows, cross-service E2E, auth, Crypt4GH, **htsget**, etc.); JSON report uploaded as an artifact. |
| **HelixTest (core services)** | Same stack, then split steps: WES + TES + DRS + TRS + Beacon, then **htsget** alone — clearer pass/fail in the Actions UI. |

Results are a **technical signal**, not official GA4GH certification (see HelixTest’s disclaimer). **Full matrix of areas covered, env vars, and how to reproduce locally:** [docs/HELIXTEST-INTEGRATION.md](docs/HELIXTEST-INTEGRATION.md).

---

## Crypt4GH: Transparent Encryption

Ferrum encrypts all data at rest with **Crypt4GH**. On download, it **re-wraps the header** for the requester’s public key — the file body is never re-encrypted.

```mermaid
sequenceDiagram
  participant Client
  participant DRS
  participant Crypt4GH as Crypt4GH Layer
  participant Storage

  Client->>DRS: GET /objects/{id}/access (Auth + X-Crypt4GH-Public-Key)
  DRS->>DRS: Auth check
  DRS->>Crypt4GH: Stream request (object_id)
  Crypt4GH->>Storage: Read encrypted object
  Storage-->>Crypt4GH: Encrypted stream (node key)
  Crypt4GH->>Crypt4GH: Decrypt header (node key)
  Crypt4GH->>Crypt4GH: Re-encrypt header (client key)
  Crypt4GH-->>DRS: Stream: new header + same body
  DRS-->>Client: Response stream
```

> **O(1) re-encryption** — Only the Crypt4GH header (typically &lt; 1 KB) is re-wrapped. The body stream is passed through with zero-copy semantics. A 500 GB BAM is re-wrapped in the same time as a 1 KB file.

See [docs/CRYPT4GH.md](docs/CRYPT4GH.md) for the full design.

---

## Deployment

### 🍎 Local demo (MacBook)

```bash
ferrum demo start
# or: make -C . demo  (from repo)
```

### 🏢 On-premises HPC

```toml
# /etc/ferrum/config.toml
bind = "0.0.0.0:8080"
[database]
url = "postgres://ferrum:***@db:5432/ferrum"
[storage]
backend = "s3"
s3_endpoint = "http://minio:9000"
s3_bucket = "ferrum"
```

```ini
# systemd: ferrum-gateway.service
ExecStart=/usr/local/bin/ferrum-gateway
Environment="FERRUM_CONFIG=/etc/ferrum/config.toml"
```

### ☸️ Kubernetes

```bash
helm repo add ferrum https://github.com/SynapticFour/Ferrum
helm install ferrum ferrum/ferrum -n ferrum --create-namespace -f values-production.yaml
```

---

## Workflow engines

| Engine | Language | Version | HPC backend |
|--------|----------|---------|-------------|
| Nextflow | Groovy/DSL2 | 24.x | SLURM, LSF |
| cwltool | CWL | 3.x | SLURM, LSF |
| Cromwell | WDL | 80+ | SLURM, LSF |
| Snakemake | Python | 8.x | SLURM, LSF |

See [docs/WORKFLOWS.md](docs/WORKFLOWS.md) for submission and DRS integration.

---

## Provenance and lineage

Ferrum tracks which WES runs consumed which DRS objects (inputs) and produced which objects (outputs), plus manual `derived_from` links on ingest. You can query **upstream** (what produced this object) or **downstream** (what used or was derived from it), view an interactive DAG in the UI, and export a run as **RO-Crate** for citation (e.g. Zenodo/Figshare). See [docs/PROVENANCE.md](docs/PROVENANCE.md).

---

## Project structure

<details>
<summary>Click to expand <code>crates/</code> tree</summary>

```
crates/
├── ferrum-core/       # Config, DB, auth, storage, error, types, health
├── ferrum-drs/        # DRS 1.4 (objects, access, ingest)
├── ferrum-trs/        # Tool Registry Service 2.0
├── ferrum-wes/        # Workflow Execution Service 1.1
├── ferrum-tes/        # Task Execution Service 1.1
├── ferrum-beacon/     # Beacon v2
├── ferrum-passports/  # GA4GH Passports & Visas
├── ferrum-crypt4gh/   # Crypt4GH encryption layer
└── ferrum-gateway/    # API gateway composing all services
```

</details>

---

## Contributing

We welcome contributions. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, testing, and the PR process.

---

## License

Licensed under the **Business Source License 1.1 (BUSL-1.1)**. See [LICENSE](LICENSE) for details. Free for non-commercial research and academic use.

---

<div align="center">
Built with ❤️ for the open science community.
Implementing GA4GH open standards for sovereign bioinformatics infrastructure.
Proudly developed by individuals on the autism spectrum in Germany 🇩🇪
We build tools that are precise, thorough, and designed to work exactly as documented.
© 2025 Synaptic Four · Licensed under BUSL-1.1 · Free for non-commercial research
</div>
