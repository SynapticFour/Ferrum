# Ferrum

Ferrum keeps genomic research files on an institution's own computers and serves them through shared interfaces, so a lab can hand a file to a collaborator without moving the collection to someone else's service.

For a research institution that runs its own servers, and for an engineer checking that setup.

**Status: Early use.** Registry: FERRUM-MATURITY-001. No third-party audit. HelixTest checks are not GA4GH certification.

[![CI](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/ci.yml)
[![NON-PILOT demo lifecycle](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/conformance.yml)
[![ferrum+infra](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml/badge.svg)](https://github.com/SynapticFour/Ferrum/actions/workflows/helixtest-ferrum-infra.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.100](https://img.shields.io/badge/rust-1.100-orange.svg)](https://www.rust-lang.org/)

The SPDX identifier is Apache-2.0.

## What it does

- One full gateway process, written in Rust, exposes seven GA4GH interfaces in its source. GA4GH is a set of shared interfaces for genomic and health data. The seven are DRS (file lookup and download), WES (workflow execution), TES (task execution), TRS (tool registry), htsget (tickets that point at sequencing files), Beacon v2 (a query for whether a dataset contains a variant), and Crypt4GH (encrypted file bytes). Code that is present is not an end-to-end test of each interface. Registry: FERRUM-GATEWAY-001.
- The edge build is the same program with a smaller set: DRS, Beacon, htsget, and Crypt4GH. It does not include WES, TES, or TRS.
- Passports are GA4GH tokens that carry a person's permissions. They are not one of the seven. The full build includes a `/passports/v1` router from this repository (`ferrum-passports`). Without a config watch, that router is mounted when Passports are enabled and sign-in is not external. `services.enable_passports` defaults to on. A config watch mounts the router as well, and the request gate then follows `enable_passports`. It does not read the external-auth flag. When sign-in is external, the process logs that Passport checks use [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra). The edge build does not include this router.
- The demo stack (`make up`) needs Postgres and object storage beside the gateway.

## Limits

- No clinical pilot deployment.
- No third-party security audit. A HelixTest run is not GA4GH certification and not a ranking against other servers.
- Beacon covers part of that standard. htsget region queries need an index object next to the data file (see docs/HTSGET.md). WES with `ferrum_backend=lsf` errors, because LSF is not implemented.
- The deployment shapes in [docs/deployment/README.md](docs/deployment/README.md), including a single computer and a cluster, are design targets. They are not a record of a production cluster in use.
- Published tags and the default branch are not stated as the same commit on this page. The tag list is the [releases page](https://github.com/SynapticFour/Ferrum/releases). Behaviour changes by version are in [CHANGELOG.md](CHANGELOG.md).

> This README describes technical capabilities, not legal advice. GDPR, NIS2, EHDS, HIPAA, and similar frameworks depend on the operator's legal basis, configuration, and organisational measures.

## Quick start

Prerequisites: Rust 1.100.0, pinned in `rust-toolchain.toml`, for the path that stays inside this repository. Docker Engine, and either the `docker compose` plugin or standalone `docker-compose`, for the demo stack. That stack also needs disk and memory for Postgres, SeaweedFS, and Keycloak.

```bash
make prove
```

You should see cargo print `test result: ok`, and at the end the line `Ferrum prove OK (OpenAPI + SPDX).`

The demo stack needs Docker:

```bash
make up
curl -sf http://127.0.0.1:8080/health
```

You should see a JSON body whose `status` is `ok` or `degraded`. `degraded` means low disk space on the configured data path, or clock skew. Stop with `make down` (keep volumes) or `make destroy` (remove volumes and project images).

`make eval` turns authentication on with HS256. It is not ga4gh-infra Passports. Stop that path with `make down-eval`.

A path that uses Passports from another repository: [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra), then `make up-pilot-local` in this repository. A laptop walkthrough of an older pinned Ferrum tag: [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo).

## Documentation

The full index, one line for every file under `docs/`, is [docs/README.md](docs/README.md). The groups are a tutorial, how-to guides, reference, and explanation.

### Tutorial

- [Getting started](docs/GETTING-STARTED.md)

### How-to guides

- [Operations](docs/OPERATIONS.md) — the email-authentication DNS check
- [Incident response](docs/INCIDENT_RESPONSE.md)
- [Metadata store](docs/METADATA-STORE.md)
- [Installation](docs/INSTALLATION.md)
- [Deployment shapes](docs/deployment/README.md) — design targets
- [Field operations](docs/FIELD-OPS.md)
- Further task pages are in the [index](docs/README.md#how-to-guides)

### Reference

- [GA4GH interfaces in this tree](docs/GA4GH.md)
- [OpenAPI dump](docs/openapi/README.md)
- [Changelog](CHANGELOG.md)
- Further reference pages are in the [index](docs/README.md#reference)

### Explanation

- [Architecture](docs/ARCHITECTURE.md)
- [Architecture decisions](docs/ARCHITECTURE-DECISIONS.md) — an index of the records in [DECISIONS.md](DECISIONS.md)
- [For evaluators](docs/FOR-EVALUATORS.md)
- [Threat model](docs/THREAT_MODEL.md)

## Licence

Ferrum is free under the Apache License, Version 2.0. The SPDX identifier is Apache-2.0. The file is [LICENSE](LICENSE). Versions released before this change were published under the Business Source License 1.1; see [LICENSING-HISTORY.md](LICENSING-HISTORY.md). The name Ferrum is not licensed with the code; see [TRADEMARKS.md](TRADEMARKS.md).

Paid are services and commitments. For Ferrum that offer is Ferrum Professional. Prices are on the [consulting page](https://synapticfour.com/en/consulting/).

## Cite

Use [CITATION.cff](CITATION.cff). Prefer that file over a hand-copied line. The related architecture report is SF-TR-2026-001, DOI [10.5281/zenodo.20628103](https://doi.org/10.5281/zenodo.20628103).

## Website

- Product page: [https://synapticfour.com/en/ferrum/](https://synapticfour.com/en/ferrum/)
- Technical profile: [https://synapticfour.com/en/technical/ferrum/](https://synapticfour.com/en/technical/ferrum/)

The same pages exist in German (`/de/ferrum/`, `/de/technik/ferrum/`) and in French (`/fr/ferrum/`, `/fr/technique/ferrum/`).

The map of repositories: [profile/README.md](https://github.com/SynapticFour/.github/blob/main/profile/README.md).

Contact: [contact@synapticfour.com](mailto:contact@synapticfour.com)
