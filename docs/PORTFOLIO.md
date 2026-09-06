# Synaptic Four portfolio

Four **products** (different buyers), plus Helix / HelixTest, HELIOS, Ferrum **companions**, and **demos**. Glue is **GA4GH** (research data/identity/compute). [Solum](https://github.com/SynapticFour/Solum) extends that into **clinical** consent, purpose, and interchange. Each repository keeps its own version and license.

Canonical copy — keep [ECOSYSTEM.md](ECOSYSTEM.md) in each public repo aligned with this map.

## Products (licensed)

| Product | What you buy | License | Stands alone |
|---------|----------------|---------|--------------|
| [Ferrum](https://github.com/SynapticFour/Ferrum) | GA4GH data/compute plane (DRS, WES, TES, TRS, Beacon, htsget, Crypt4GH) | BUSL-1.1 | Yes — built-in passports |
| [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra) (**GA4GH Infra**) | Identity plane (OIDC broker, Passports/visas, DUO, ADS, service registry) | Apache-2.0 | Yes — Compose stack |
| [Solum](https://github.com/SynapticFour/Solum) | Clinical compliance overlay (consent, audit chain, FHIR IPS, field crypto) | BUSL-1.1 | Yes — Ferrum pin is optional crypto/types |
| [BioResearch Assistant](https://github.com/SynapticFour/bioresearch-assistant) | Researcher workbench (phenopackets, literature, jobs) | BUSL-1.1 | Yes — own DRS/WES if no Ferrum |

**ga4gh-infra is Apache on purpose (open-core):** institutes can run the identity plane against Ferrum (or another implementer). Commercial motion is Ferrum / Solum / BRA licenses plus optional support. See [ga4gh-infra IDENTITY](https://github.com/SynapticFour/ga4gh-infra/blob/main/docs/IDENTITY.md).

## Open source (Apache-2.0)

| Repo | Role |
|------|------|
| [HelixTest](https://github.com/SynapticFour/HelixTest) | GA4GH conformance CLI (`helixtest`) against a running target. Suite pin **v0.1.3**. |
| [Helix](https://github.com/SynapticFour/Helix) | VERIFY CLI around HelixTest (`helix verify` / `security` / `bench`). Separate git root, distinct from HELIOS. |
| [HELIOS](https://github.com/SynapticFour/HELIOS) | Signed pipeline/audit evidence (`helios-audit` on PyPI). File ingest — does **not** orchestrate Ferrum or Solum |

CI wrappers: [helixtest-action](https://github.com/SynapticFour/helixtest-action) (default binaries **v0.1.3**) and [helix-action](https://github.com/SynapticFour/helix-action) (`helix verify` from Helix + HelixTest **v0.1.3**). HelixTest validates against **published GA4GH OpenAPI**; Ferrum’s utoipa dump is an implementation map only.

## Ferrum companions

| Repo | Role |
|------|------|
| [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) | Select 1–3 GA4GH surfaces; generate Compose/Helm/systemd around the Ferrum image |
| [ferrum-meta](https://github.com/SynapticFour/ferrum-meta) | LinkML schema + archive profiles (GHGA/EGA/…) for interchange; runtime lives in Ferrum |

## Demos

Local walkthroughs so you can see real behaviour on a laptop.

| Repo | Role |
|------|------|
| [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo) | `./run` laptop pipeline walkthrough (DRS/WES/TES) |
| [Solum-Demo](https://github.com/SynapticFour/Solum-Demo) | Local Stage-1 consent/audit walkthrough |
| [SynapticFour-Showcase](https://github.com/SynapticFour/SynapticFour-Showcase) | Evidence pack + pin checkout |

Tests live **in each product** (`make prove`). There is no Solum-Test repo. HelixTest is the **external** GA4GH proof.

**Toolchain:** Ferrum, Lab Kit, ga4gh-infra, Solum, Helix, and HelixTest checkouts use **Rust 1.91.1** (`rust-toolchain.toml`). HelixTest **MSRV** remains **1.88**. BRA and HELIOS require **Python 3.11+**.

**Support:** written commercial license is [COMMERCIAL.md](COMMERCIAL.md). ga4gh-infra is Apache-2.0 (support optional). Lab Kit generates deploy files around Ferrum. Helix, HelixTest, and HELIOS are Apache-2.0.

## Optional joins (standards, not a monorepo)

| From → to | Glue |
|-----------|------|
| Ferrum → ga4gh-infra | Passports / clearinghouse (`[auth] mode = "external"`) |
| Ferrum → Solum | Consent-gated DRS/WES (HTTP, fail-closed) |
| BRA → Ferrum | GA4GH DRS/WES client when `FERRUM_DRS_URL` / `FERRUM_WES_URL` are set. Contract is the published DRS/WES OpenAPI |
| BRA → Solum | Optional subject-link HTTP |
| HELIOS → Solum | JSON export file on disk |
| Lab Kit → Ferrum | SHA-pinned image + `FERRUM_SERVICES__ENABLE_*` |
| HelixTest → Ferrum / infra | HTTP conformance against a running stack; schemas are the **published GA4GH OpenAPI** (vendored). Ferrum [utoipa dump](openapi/ferrum.openapi.json) is gateway/extension map only |
| Helix → HelixTest | Cargo path deps on sibling crates; CI pin `HELIXTEST_SHA` = v0.1.3. Separate git roots. |

See each repo `docs/IDENTITY.md` for audience / not-for / standalone prove.
