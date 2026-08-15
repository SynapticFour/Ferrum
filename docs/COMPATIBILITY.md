# Compatibility pins (composition, not a monorepo)

Each product versions independently. This table is the **last combination we claim works together**. Consumers that import another repo must pin these refs — not `main`.

| Product | Role | Pin (15 Aug 2026) |
|---------|------|-------------------|
| Ferrum | Data/compute | tag **v0.3.0** (`6444469a`) |
| ga4gh-infra | Identity | tag **ga4gh-infra-v0.2.2** (`e43bf08`); Compose/GHCR stack images **`:0.2.2`** (crate Cargo.toml may stay 0.1.0) |
| Solum | Clinical library | consumes ferrum-core **v0.3.0** (`6444469a` in Solum `config/ci/ferrum-revision.txt`) |
| Solum-Demo / Lab-Kit sidecar | Runnable clinical demo | Solum SHA **6b4519c** (verified demo baseline; older than Solum HEAD after the v0.3.0 consume bump) |
| HelixTest | Conformance | tag **v0.1.1** (Ferrum `VERSIONS.lock`) |
| ferrum-meta | Schema plane | SHA **e34fce4** (`FERRUM_META_SHA`; vendored into Ferrum `profiles/meta/schema/`) |
| HELIOS | Signed evidence | HEAD of HELIOS at demo time; no required Ferrum API |
| BRA | Researcher UI | optional HTTP to Solum; no Ferrum crate pin |

Source of truth for Ferrum×infra×HelixTest: this repo’s [`VERSIONS.lock`](../VERSIONS.lock). Solum-Demo and Lab-Kit stay on Solum **6b4519c** until Solum cuts a `v*` tag — that demo pin is independent of which Ferrum SHA Solum HEAD compiles against.

Identity of each product: [IDENTITY.md](IDENTITY.md).
