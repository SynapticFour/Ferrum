# Synaptic Four portfolio

Four **products** (different buyers), two **free ambassadors**, Ferrum **companions**, and **proof** repos. Glue is **GA4GH** (research data/identity/compute). [Solum](https://github.com/SynapticFour/Solum) extends that into **clinical** consent, purpose, and interchange. This is a portfolio, **not a bundle SKU**.

Canonical copy — keep [ECOSYSTEM.md](ECOSYSTEM.md) in each public repo aligned with this map.

## Products (licensed)

| Product | What you buy | License | Stands alone |
|---------|----------------|---------|--------------|
| [Ferrum](https://github.com/SynapticFour/Ferrum) | GA4GH data/compute plane (DRS, WES, TES, TRS, Beacon, htsget, Crypt4GH) | BUSL-1.1 | Yes — built-in passports |
| [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra) | Identity plane (OIDC broker, Passports/visas, DUO, ADS, service registry) | Apache-2.0 | Yes — Compose stack |
| [Solum](https://github.com/SynapticFour/Solum) | Clinical compliance overlay (consent, audit chain, FHIR IPS, field crypto) | BUSL-1.1 | Yes — Ferrum pin is optional crypto/types |
| [BioResearch Assistant](https://github.com/SynapticFour/bioresearch-assistant) | Researcher workbench (phenopackets, literature, jobs) | BUSL-1.1 | Yes — own DRS/WES if no Ferrum |

**ga4gh-infra is Apache on purpose (open-core):** institutes can run the identity plane against Ferrum (or another implementer) without a proprietary lock on Passports. Commercial motion is Ferrum / Solum / BRA licenses plus optional support — not a second closed identity SKU. See [ga4gh-infra IDENTITY](https://github.com/SynapticFour/ga4gh-infra/blob/main/docs/IDENTITY.md).

## Ambassadors (free — Apache-2.0)

High public quality bar. Not sold. They make Synaptic Four visible.

| Repo | Role |
|------|------|
| [HelixTest](https://github.com/SynapticFour/HelixTest) | GA4GH conformance CLI against a running target (not a server) |
| [HELIOS](https://github.com/SynapticFour/HELIOS) | Signed pipeline/audit evidence (`helios-audit` on PyPI). File ingest — does **not** orchestrate Ferrum or Solum |

## Comes with Ferrum (not sold separately)

| Repo | Role |
|------|------|
| [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) | Select 1–3 GA4GH surfaces; generate Compose/Helm/systemd around the Ferrum image |
| [ferrum-meta](https://github.com/SynapticFour/ferrum-meta) | LinkML schema + archive profiles (GHGA/EGA/…) for interchange; runtime lives in Ferrum |

## Proof / outreach (not sold)

Local walkthroughs so a curious operator can see real behaviour. Not evaluation kits, not pilots.

| Repo | Role |
|------|------|
| [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo) | `./run` laptop pipeline smoke (DRS/WES/TES) |
| [Solum-Demo](https://github.com/SynapticFour/Solum-Demo) | Local Stage-1 consent/audit walkthrough |
| [SynapticFour-Showcase](https://github.com/SynapticFour/SynapticFour-Showcase) | Evidence pack + pin checkout; not a product |

Tests live **in each product** (`make prove`). There is no Solum-Test repo. HelixTest is the **external** GA4GH proof.

## Optional joins (standards, not a monorepo)

| From → to | Glue |
|-----------|------|
| Ferrum → ga4gh-infra | Passports / clearinghouse (`[auth] mode = "external"`) |
| Ferrum → Solum | Consent-gated DRS/WES (HTTP, fail-closed) |
| BRA → Ferrum | Intended: GA4GH DRS/WES client when Ferrum is present (not yet the default) |
| BRA → Solum | Optional subject-link HTTP |
| HELIOS → Solum | JSON export file on disk |
| Lab Kit → Ferrum | SHA-pinned image + `FERRUM_SERVICES__ENABLE_*` |
| HelixTest → Ferrum / infra | HTTP conformance against a running stack |

See each repo `docs/IDENTITY.md` for audience / not-for / standalone prove.
