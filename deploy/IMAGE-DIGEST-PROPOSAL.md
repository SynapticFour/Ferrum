# Compose image digest proposal

Compose files were not edited. Index digests below were resolved with
`docker buildx imagetools inspect` on 2026-10-14.

## Already digest-pinned (no proposal)

`deploy/docker-compose.yml` and `demo/docker-compose.demo.yml` already pin
Postgres, SeaweedFS, Keycloak, nginx, and the demo GHCR Ferrum images with
`@sha256`.

## Local build tags with no registry digest

These names are built by Compose. They are not registry pulls, so no index
digest was resolved for them.

| Compose image | Current default | File |
|---|---|---|
| `ferrum-init:${FERRUM_VERSION:-latest}` | `:latest` when `FERRUM_VERSION` is unset | `deploy/docker-compose.yml` |
| `ferrum-gateway:${FERRUM_VERSION:-latest}` | `:latest` when `FERRUM_VERSION` is unset | `deploy/docker-compose.yml` |
| `ferrum-ui:${FERRUM_VERSION:-latest}` | `:latest` when `FERRUM_VERSION` is unset | `deploy/docker-compose.yml` |

## `deploy/docker-compose.ga4gh-infra.yml`

Defaults in that file are tag `0.2.6` and prefix `ghcr.io/synapticfour`.

| Compose image | Tag inspected | Index digest |
|---|---|---|
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/mock-idp:${MOCK_IDP_VERSION:-0.2.6}` | `ghcr.io/synapticfour/mock-idp:0.2.6` | `sha256:3d87a960da4c8a841810cdefe9b1343426b4d973f0670119d85214e308c45eab` |
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/visa-registry:${VISA_REGISTRY_VERSION:-0.2.6}` | `ghcr.io/synapticfour/visa-registry:0.2.6` | `sha256:d438db97904eb331d1cf0d4d7b09593370bf5923422c5f0a1262ec6a2465514a` |
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/duo-service:${DUO_SERVICE_VERSION:-0.2.6}` | `ghcr.io/synapticfour/duo-service:0.2.6` | `sha256:0135a835359fc9a0f87ce846593b3ba5374b67d53b2b8f2af114d59ce3c3dc52` |
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/service-registry:${SERVICE_REGISTRY_VERSION:-0.2.6}` | `ghcr.io/synapticfour/service-registry:0.2.6` | `sha256:a5b7b07a74b8128d67043d13c8d3e0feb91e07ac2de2e7aedea4312da95b36e7` |
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/access-decision-service:${ADS_VERSION:-0.2.6}` | `ghcr.io/synapticfour/access-decision-service:0.2.6` | `sha256:dfbcf7f67b9d28d5333c0921ad2b8477b8f6215cf5ff9e9e5bf7b24d542aa701` |
| `${GA4GH_IMAGE_PREFIX:-ghcr.io/synapticfour}/aai-broker:${AAI_BROKER_VERSION:-0.2.6}` | `ghcr.io/synapticfour/aai-broker:0.2.6` | `sha256:c12990141b975f2ad7a33a2268d0dc67a79c5a20f4606aa806d03acfbb9e4589` |
