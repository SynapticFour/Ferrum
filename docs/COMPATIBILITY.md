# Compatibility pins (composition, not a monorepo)

Each product versions independently. This table is the **last combination we claim works together**. Consumers that import another repo must pin these refs — not `main`.

| Product | Role | Pin (18 Aug 2026) |
|---------|------|-------------------|
| Ferrum | Data/compute | tag **v0.3.2** / `2bd147c` (`VERSIONS.lock` `FERRUM_VERSION`) |
| ga4gh-infra | Identity | tag **ga4gh-infra-v0.2.3** (`613bd14`); Compose/GHCR stack images **`:0.2.3`** (crate Cargo.toml may stay 0.1.0) |
| Solum | Clinical library | tag **v0.1.0** / `b68a941`; consumes ferrum-core **v0.3.2** (`2bd147c` in Solum `config/ci/ferrum-revision.txt`) |
| Solum-Demo | Runnable clinical demo | Solum **v0.1.0** (`PINNED_VERSIONS.txt` `Solum-ref`) |
| HelixTest | Conformance | tag **v0.1.3** / SHA **1832c043e167** (Ferrum `HELIXTEST_SHA`) |
| ferrum-meta | Schema plane | tag **v0.1.0** / SHA **e34fce4** (`FERRUM_META_SHA`; vendored into Ferrum `profiles/meta/schema/`). Do not vendor `main`. |
| HELIOS | Signed evidence | tag **v0.1.1**; reads export JSON from disk; no required Ferrum API |
| BRA | Researcher UI | tag **v0.2.1**; optional HTTP to Ferrum/Solum; no Ferrum crate pin |

Source of truth for Ferrum×infra×HelixTest×meta: this repo’s [`VERSIONS.lock`](../VERSIONS.lock). A product `main` may be ahead of this join.

## Release train

A **Ferrum tag** is the composition contract. After this table and `VERSIONS.lock` change:

1. Ferrum CI green against the new `GA4GH_INFRA_REF` (optional `clearinghouse` feature).
2. Lab-Kit Compose image tags follow the infra stack version (already `:0.2.3`).
3. Showcase `PINNED_VERSIONS.txt` only after published artefacts are regenerated.
4. Demos (Ferrum-GA4GH-Demo, Solum-Demo) follow **one repo at a time**.

Do not cut ten sibling tags on the same day. Optional joins (Passports, Solum HTTP, BRA subject-link) stay optional.

Identity of each product: [IDENTITY.md](IDENTITY.md).
