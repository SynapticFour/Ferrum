# Deployment shapes

What each shape **needs**, and what is **verified**. Helix baseline: HelixTest already runs; Helix productizes conformance/security/benchmark; evidence stays HELIOS. Green CI is not GA4GH certification. No clinical pilot deployment is claimed.

## Shapes

| Shape | Processes / deps | Auth / TES | Verified? |
|-------|------------------|------------|-----------|
| **Edge** | **One** Ferrum gateway process; SQLite (or embedded DB path); **local files** for objects (`FERRUM_STORAGE__BACKEND=local`). No Postgres, no object store, no IdP required. | Operator config; often offline JWKS cache | `make edge` / native binary paths exist. **Offline edge install from the GitHub offline bundle: documented-not-verified** until someone runs `import_offline_bundle` + edge on air-gapped hardware and records evidence. |
| **Demo / laptop (`make up`)** | Gateway + **Postgres** + **S3 object store** (reference: SeaweedFS) + Keycloak + UI + init + nginx. | Auth **off**; TES **noop**; HelixTest stubs **on** | CI HelixTest / microbench against this Compose family. |
| **Demo + real TES (`make up-tes`)** | Demo stack + Docker TES backend. | inherits demo | CI `test-tes` when stack is up. |
| **Full / pilot (`make up-pilot-local`)** | Demo deps + **ga4gh-infra** (broker, visa-registry, ADS, …) + mock-idp. | Passport / JWKS | Local pilot CI jobs; not a clinical pilot. |
| **Helm / k8s** | Gateway Deployment; **operator-supplied** Postgres + S3 (+ optional IdP). Chart does not install Postgres/Seaweed/MinIO. | values-driven | Chart packages; cluster bring-up is operator-verified. |

## “One gateway” means

The **full** gateway is one HTTP process for DRS, WES, TES, TRS, Beacon, htsget, Passports, and Crypt4GH — not “zero dependencies.”

- **Edge binary:** DRS, Beacon, htsget, and Crypt4GH, in one process with local disk and SQLite. It does not serve WES, TES, TRS, or Passports.
- **Full demo:** the full gateway process **plus** Postgres and object storage (and usually an IdP/broker for Passports).

## Reaching an edge node from the network

The edge profile listens on `127.0.0.1:8080` when neither a config file nor `FERRUM_BIND` sets `bind`. The full profile stays `0.0.0.0:8080`. A process bound only to loopback inside a container is not reached by a published port or by another container's DNS name. Set `FERRUM_BIND` (for example `0.0.0.0:8080` inside a container) and control exposure with the published port or a firewall. Ferrum-Lab-Kit sets `FERRUM_BIND` explicitly. The Demo Pi installer and village compose need the same explicit bind (see `_review/edge-bind-usage.md`).

## “All APIs in one offline binary”

**Not verified** as a single claim. The release ships gateway tarballs and (optionally) an offline *image* bundle for the Compose demo shape. The edge binary serves DRS, Beacon, htsget, and Crypt4GH with local storage. It does not serve WES, TES, TRS, or Passports, and it is not the same artifact as the full Compose offline tarball. Treat “offline” as shape-specific until an operator records a successful air-gapped run ([documented-not-verified](#shapes) for edge offline bundle).

## Object storage

Reference Compose uses **SeaweedFS** (Apache-2.0). Garage (AGPL-3.0) and MinIO (AGPL-3.0, Hub/binary currently unusable anonymously) are BYO / alternatives — [TESTED-S3-BACKENDS.md](TESTED-S3-BACKENDS.md).

## Prerequisites (`make up`)

- Docker Engine
- Compose: **`docker compose`** (plugin) **or** standalone **`docker-compose`** — Makefile detects either via `deploy/scripts/compose-cmd.sh`
- Enough RAM/disk for Postgres + SeaweedFS + Keycloak + gateway builds
