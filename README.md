# Ferrum

[![CI](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml)
[![NON-PILOT demo lifecycle](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml)
[![ferrum+infra](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)
[![Rust 1.91](https://img.shields.io/badge/rust-1.91-orange.svg)](https://www.rust-lang.org/)

On-premises GA4GH data/compute plane in Rust: DRS, WES, TES, TRS, Beacon v2, htsget, Passports, Crypt4GH — one gateway.

**Maturity: Active (beta).** One maintainer. No third-party audit. HelixTest CI is **NON-PILOT** (auth off, TES noop, stubs on) and is not GA4GH certification. Latest tag **v0.3.2**. Suite consumers still pin **v0.3.1** until that join is bumped.

> This README describes technical capabilities, not legal advice. GDPR, NIS2, EHDS, HIPAA, and similar frameworks depend on the operator’s legal basis, configuration, and organisational measures.

## Ferrum / GA4GH suite

These ten public repositories are from the same organisation and can be composed. They are not a fifth product and not a bundle SKU. Each repository keeps its own version and license. Roles, maturity, and who consumes whom: [SUITE-OVERVIEW](https://github.com/SynapticFour/Ferrum/blob/main/docs/SUITE-OVERVIEW.md).

## Quick start

```bash
make prove    # cargo test --workspace --all-targets (no Docker)
make eval     # require_auth=true (HS256). TES noop. Not ga4gh-infra Passports.
make up       # demo — TES noop; auth off (NON-PILOT). Not a pilot.
make up-tes   # demo + Docker-backed TES (local containers; not default)
```

Stop: `make down` (keep volumes) or `make destroy` (remove volumes and project images).

htsget region queries (`referenceName` / `start` / `end`) return HTTP 400. WES `ferrum_backend=lsf` errors (LSF is not implemented).

## Documentation

- [Getting started](docs/GETTING-STARTED.md) — installer CLI, eval vs demo vs TES, uninstall
- [Architecture](docs/ARCHITECTURE.md)
- [For evaluators](docs/FOR-EVALUATORS.md)
- [GA4GH specs vs this implementation](docs/GA4GH.md) · [OpenAPI dump](docs/openapi/README.md)
- [Operator trust](docs/OPERATOR-TRUST.md) · [Documentation index](docs/README.md)

Specialist notes (TES Docker, MII Connect, field/edge, and others) stay under `docs/` and are linked from the [index](docs/README.md). They are annexes, not the front door.

## License

Business Source License 1.1 — see [LICENSE](LICENSE).
