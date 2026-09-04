# Synaptic Four — this repo in the portfolio

Four **products**, two ambassador lines (**Helix / HelixTest**, **HELIOS**), Ferrum **companions**, and **reference / demo** repos. Glue is GA4GH; Solum extends into clinical data. **Not a bundle SKU.** Canonical public map: [SUITE-OVERVIEW.md](SUITE-OVERVIEW.md). Longer portfolio notes: [PORTFOLIO.md](PORTFOLIO.md).

**You are here:** [Ferrum](https://github.com/SynapticFour/Ferrum) — data/compute plane (DRS, WES, TES, TRS, Beacon, htsget, Crypt4GH).

## Repositories

| Kind | Repository | Role | License |
|------|------------|------|---------|
| Product | **Ferrum** (this repo) | GA4GH gateway and services | BUSL-1.1 |
| Product | [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra) | Identity plane (Apache open-core) | Apache-2.0 |
| Product | [Solum](https://github.com/SynapticFour/Solum) | Clinical overlay | BUSL-1.1 |
| Product | [BioResearch Assistant](https://github.com/SynapticFour/bioresearch-assistant) | Researcher workbench | BUSL-1.1 |
| Ambassador | [HelixTest](https://github.com/SynapticFour/HelixTest) | GA4GH conformance CLI (`helixtest`) | Apache-2.0 |
| Ambassador | [Helix](https://github.com/SynapticFour/Helix) | VERIFY brand around HelixTest (`helix verify`) | Apache-2.0 |
| Ambassador | [HELIOS](https://github.com/SynapticFour/HELIOS) | Pipeline audit evidence (file ingest) | Apache-2.0 |
| With Ferrum | [Ferrum-Lab-Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) | Subset install — Ferrum companion | BUSL-1.1 |
| With Ferrum | [ferrum-meta](https://github.com/SynapticFour/ferrum-meta) | Metadata interchange schemas | Apache-2.0 |
| Proof | [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo) | Local `./run` smoke | Apache-2.0 |

## Ownership boundaries

| Layer | Owner | Notes |
|-------|--------|--------|
| Identity | **ga4gh-infra** | Broker, visas, DUO, ADS, service registry |
| Data/compute | **Ferrum** | DRS, WES/TES, TRS, Beacon; built-in passports in standalone mode; optional Metadata Store (`[metadata_store]`) |
| Schema (scientific meta) | **[ferrum-meta](https://github.com/SynapticFour/ferrum-meta)** | LinkML profiles; runtime store/API in Ferrum (ADR-025) |
| Deployment | **Ferrum-Lab-Kit** | Selective GA4GH surfaces for labs; does not fork Ferrum |
| Demo/benchmark | **Ferrum-GA4GH-Demo** | Reproducible GIAB benchmark; optional `--with-infra` |
| Conformance | **HelixTest** (tagged CLI) / **Helix** (VERIFY brand) | Automated API and workflow tests. Ferrum `main` runs HelixTest v0.1.3. |

Co-deploy: Ferrum `[auth] mode = "external"` disables built-in `/passports/v1` and validates via `ga4gh-clearinghouse`. See [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md) and [DECISIONS.md](../DECISIONS.md) (ADR-017).

## Default co-deploy ports

| Service | Standalone Ferrum | Co-deploy (demo / lab) |
|---------|-------------------|-------------------------|
| Ferrum gateway | 8080 | 18080 (demo) or 8080 (lab) |
| AAI broker | — | 8180 |
| Visa registry | — | 8181 |
| DUO | — | 8182 |
| Service registry | — | 8183 |
| ADS | — | 8190 |
| mock-idp | — | 9100 |

## Local lifecycle (unified commands)

Repos that run a **local Docker stack** share the same verbs:

| Verb | Meaning |
|------|---------|
| **up** | Install (if needed) and start |
| **down** | Stop containers; **keep volumes** |
| **destroy** | Stop containers and **remove volumes** |

| Repository | Deploy | Stop | Destroy | Notes |
|------------|--------|------|---------|-------|
| **ga4gh-infra** | `make up` / `just up` | `make down` | `make destroy` | Native binary: [getting-started.md](getting-started.md) |
| **Ferrum** | `make up` / `ferrum demo start` | `make down` | `make destroy` | Edge: `ferrum demo start --edge` |
| **Ferrum-Lab-Kit** | `make up` | `make down` | `make destroy` | Co-deploy: `make up-with-infra` |
| **Ferrum-GA4GH-Demo** | `make up` / `./run` | `make down` | `make destroy` | Co-deploy: `make up-with-infra` |
| **HelixTest** | — | — | — | Conformance runner (needs a running target) |

**Multi-repo co-deploy** (Ferrum + ga4gh-infra):

```bash
# Benchmark path (Demo)
cd Ferrum-GA4GH-Demo && make up-with-infra
make down        # or make destroy

# Field edge path (Lab Kit)
cd Ferrum-Lab-Kit && make up-with-infra
make down        # or make destroy
```

Secondary options (always available): repo `scripts/stack-*.sh`, raw `docker compose`, and paths documented in each README.

## Quick starts

**Benchmark + co-deploy (demo):**

```bash
export FERUM_SRC=/path/to/Ferrum
# or: export FERRUM_SRC=/path/to/Ferrum   # alias accepted by Demo
export GA4GH_INFRA_SRC=/path/to/ga4gh-infra
cd Ferrum-GA4GH-Demo && ./run --with-infra
# Evidence: make smoke-evidence · coverage: docs/COVERAGE.md
```

**Field edge + infra (lab / Pi hub):**

```bash
# Pi or field node (from Ferrum repo)
./scripts/install-field-edge.sh [--with-infra]

# Or via Lab Kit profile field-edge
cd Ferrum-Lab-Kit && ./install-edge.sh --profile field-edge
```

See [FIELD-ECOSYSTEM.md](FIELD-ECOSYSTEM.md) and [FIELD-GA4GH-DEMO-PI.md](FIELD-GA4GH-DEMO-PI.md).

**Conformance:**

```bash
helixtest --all --mode ferrum
helixtest --all --mode ferrum+infra --profile ferrum-infra
```

## Documentation map

| Topic | Document |
|-------|----------|
| Ferrum ↔ ga4gh-infra wiring | [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md) |
| Demo compose merge order | [Ferrum-GA4GH-Demo `docs/architecture.md`](https://github.com/SynapticFour/Ferrum-GA4GH-Demo/blob/main/docs/architecture.md) |
| Lab co-deploy profiles | [Ferrum-Lab-Kit `config/profiles/field-edge+infra.toml`](https://github.com/SynapticFour/Ferrum-Lab-Kit/blob/main/config/profiles/field-edge+infra.toml) |
| HelixTest co-deploy mode | [HelixTest `helixtest/docs/ferrum.md`](https://github.com/SynapticFour/HelixTest/blob/main/helixtest/docs/ferrum.md) |
| Africa-Mode (Edge / SQLite) | [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-ECOSYSTEM.md](FIELD-ECOSYSTEM.md) |
| Solum clinical companion | [Solum](https://github.com/SynapticFour/Solum) — consent teeth + subject bridge |
| Subject bridge operator runbook | [solum-subject-bridge-runbook.md](solum-subject-bridge-runbook.md) |

## CI

Each repository runs GitHub Actions on `main`. Ferrum additionally runs HelixTest conformance and Africa scenarios. Path dependencies on `ga4gh-infra` are resolved in CI via `.github/actions/clone-ga4gh-infra` and in Dockerfiles via a shallow `git clone` of `ga4gh-infra`.
