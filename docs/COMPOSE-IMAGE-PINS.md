# Compose image pins (digests)

Pinned on 2026-10-12 from `docker pull` (linux/amd64 for GHCR / SeaweedFS).

## `deploy/docker-compose.yml` (demo / `make up`)

| Service | Image |
|---------|--------|
| postgres | `postgres:16-alpine@sha256:721873c34ceb9f8d8fc265984940dc982404c105f19ad51be9fdc5970a6080ea` |
| object-store | `chrislusf/seaweedfs:3.80@sha256:1055999e08eed1789b0ae45d235126e4495e23d3fb9d6396293fd42539b1ae6a` (`platform: linux/amd64`) |
| keycloak | `keycloak/keycloak:26.0@sha256:09a381c715ab0b111835b70f2905955274843a219c6f27efb348e4d9f4086858` |
| nginx | `nginx:alpine@sha256:df221db836e1754089190208cee7eeda94f233197056426eda74a43ab1abeac2` |
| ferrum-gateway / ferrum-ui / ferrum-init | **Built locally** (`pull_policy: build`) — no registry digest |

Dockerfile build args (when set): `rust:1.91.1-alpine@sha256:45c1c35…`, `rust:1.91.1-bookworm@sha256:c1e5f19…`, `debian:bookworm-slim@sha256:7c7b2c…`, `alpine:3.19@sha256:6baf435…`.

## `demo/docker-compose.demo.yml`

| Service | Image |
|---------|--------|
| postgres | same as above |
| gateway | `ghcr.io/synapticfour/ferrum:v0.3.5-full-infra@sha256:555f44bada386bd58d7b6914a94b22e4bd5319edf5d4e70c11c7a82dae14c312` |
| ui | `ghcr.io/synapticfour/ferrum-ui:v0.3.5@sha256:59b7230e2736c089d81ca7316cde811a4da33658cfe2297b4e43e905c51ae2bf` |
| nginx | same as above |

## `deploy/docker-compose.ga4gh-infra.yml`

Default tags are **0.2.6**. `pull_policy: build` rebuilds from `GA4GH_INFRA_SRC` (sibling checkout) — **no digest on the running container**. Published digests for pull-only use:

| Image | Digest |
|-------|--------|
| `aai-broker:0.2.6` | `sha256:c12990141b975f2ad7a33a2268d0dc67a79c5a20f4606aa806d03acfbb9e4589` |
| `visa-registry:0.2.6` | `sha256:d438db97904eb331d1cf0d4d7b09593370bf5923422c5f0a1262ec6a2465514a` |
| `mock-idp:0.2.6` | `sha256:3d87a960da4c8a841810cdefe9b1343426b4d973f0670119d85214e308c45eab` |
| `duo-service:0.2.6` | `sha256:0135a835359fc9a0f87ce846593b3ba5374b67d53b2b8f2af114d59ce3c3dc52` |
| `service-registry:0.2.6` | `sha256:a5b7b07a74b8128d67043d13c8d3e0feb91e07ac2de2e7aedea4312da95b36e7` |
| `access-decision-service:0.2.6` | `sha256:dfbcf7f67b9d28d5333c0921ad2b8477b8f6215cf5ff9e9e5bf7b24d542aa701` |

## Exceptions (not digest-pinned in Compose)

| Exception | Why |
|-----------|-----|
| `ferrum-gateway`, `ferrum-ui`, `ferrum-init` in repo Compose | Local `build:` / `pull_policy: build` |
| ga4gh-infra services under `up-pilot-local` | Built from sibling source; digests listed above for pull-only |
| Mutable tags without `@sha256` elsewhere | Must not be introduced; prefer digest |
| MinIO | Not in reference Compose (BYO S3 only) |

Re-pin after intentional upgrades: `docker pull --platform linux/amd64 <image:tag>` and copy `Digest:`.
