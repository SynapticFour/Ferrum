# Ferrum

[![CI](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml)
[![NON-PILOT demo lifecycle](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml)
[![ferrum+infra](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)

GitHub may label this Other; SPDX is **BUSL-1.1**.
[![Rust 1.91](https://img.shields.io/badge/rust-1.91-orange.svg)](https://www.rust-lang.org/)

On-premises GA4GH data/compute plane in Rust: DRS, WES, TES, TRS, Beacon v2, htsget, Passports, Crypt4GH — **one HTTP gateway process** (not zero dependencies: full demo needs Postgres + object storage).

**Maturity: Active (beta).** No third-party audit. HelixTest CI is **NON-PILOT** (auth off, TES noop, stubs on) and is not GA4GH certification. Git tag **v0.3.6** is on origin and is the same commit as `origin/main` (checked 2026-10-14; SeaweedFS reference store, compose digests). It is not a proposed tag. A GitHub Release object for that tag was not re-checked in this note. No clinical pilot deployment. The edge binary serves DRS, Beacon, htsget, and Crypt4GH. It does not serve WES, TES, TRS, or Passports. Those four are in the full gateway, which is a different build.

> This README describes technical capabilities, not legal advice. GDPR, NIS2, EHDS, HIPAA, and similar frameworks depend on the operator’s legal basis, configuration, and organisational measures.

These public repositories are maintained by the same organisation and are designed to work together. Each repository keeps its own version and license. For details on roles, maturity, and how the components relate to one another, see [SUITE-OVERVIEW](https://github.com/SynapticFour/.github/blob/main/profile/SUITE-OVERVIEW.md).

## Quick start

Laptop walkthrough of **tagged** Ferrum (DRS / WES / TES): [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo) `./run` (gateway `http://127.0.0.1:18080`). Org stranger path: [github.com/SynapticFour](https://github.com/SynapticFour#try-it-30-min).

**Prerequisites for `make up`:** Docker Engine, and either the `docker compose` plugin **or** standalone `docker-compose` (Makefile detects both). Disk/RAM for Postgres + SeaweedFS + Keycloak + image builds.

```bash
make prove    # cargo test --workspace --all-targets (no Docker)
make eval     # require_auth=true (HS256). TES noop.
make up       # demo stack — TES noop; auth off; Postgres + SeaweedFS S3
make up-tes   # demo + Docker-backed TES (local containers)
```

Stop: `make down` (keep volumes) or `make destroy` (remove volumes and project images).

htsget region queries (`referenceName` / `start` / `end`) return HTTP 400. WES `ferrum_backend=lsf` errors (LSF is not implemented).

## Documentation

- [Deployment shapes](docs/DEPLOYMENT-SHAPES.md) — edge vs full; what is verified
- [Tested S3 backends](docs/TESTED-S3-BACKENDS.md) — SeaweedFS reference; Garage/MinIO BYO
- [Compose image pins](docs/COMPOSE-IMAGE-PINS.md)
- [Getting started](docs/GETTING-STARTED.md) — installer CLI, eval vs demo vs TES, uninstall
- [Architecture](docs/ARCHITECTURE.md)
- [For evaluators](docs/FOR-EVALUATORS.md)
- [GA4GH specs vs this implementation](docs/GA4GH.md) · [OpenAPI dump](docs/openapi/README.md)
- [Operator trust](docs/OPERATOR-TRUST.md) · [Documentation index](docs/README.md)

Specialist notes (TES Docker, MII Connect, field/edge, and others) live under `docs/` and are linked from the [index](docs/README.md).

## License

Business Source License 1.1 — see [LICENSE](LICENSE).

**Synaptic Four** · [contact@synapticfour.com](mailto:contact@synapticfour.com) · [synapticfour.com](https://synapticfour.com)
