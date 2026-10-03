# Ferrum gateway image variants

Lab Kit and operators should **select a published variant**, not compile an ad-hoc matrix of every GA4GH combination. Runtime `FERRUM_SERVICES__ENABLE_*` still hides unused routes inside a variant.

## Published tags (`ghcr.io/synapticfour/ferrum`)

| Variant | Cargo | Surfaces compiled in | GHCR tags |
|---------|--------|----------------------|-----------|
| **full** | default (`full`) | Beacon, DRS, htsget, WES, TES, TRS, Passports, discovery, … | `:<sha>`, `:<sha>-full`, `:latest`, `:full`, version tags |
| **edge** | `--no-default-features --features edge` | DRS + Beacon + htsget + Crypt4GH on SQLite | `:<sha>-edge`, `:edge` |
| **edge-infra** | `--features edge,external-auth` | edge + ga4gh-infra clearinghouse + service-registry | `:<sha>-edge-infra`, `:edge-infra` |

`:<sha>` without a suffix is always **full** (backward compatible).

htsget is part of **edge** at compile time; Lab Kit `field-edge` still sets `FERRUM_SERVICES__ENABLE_HTSGET=false` so the route is off. A Beacon+DRS-only **binary** would need optional `ferrum-htsget` — not a Lab Kit Dockerfile.

## Local / custom architecture

GHCR images from `deploy/Dockerfile` are **linux/amd64** musl/distroless unless a multi-arch manifest is published. For Raspberry Pi, Apple Silicon native arm64, or air-gap:

```bash
./scripts/build-variant-image.sh --variant edge --platform linux/arm64 --tag ferrum:edge-local
```

Ferrum-Lab-Kit wraps the same script (`lab-kit build image`). Override cargo features only when you know the gateway feature flags:

```bash
./scripts/build-variant-image.sh --features edge,external-auth --platform linux/arm64
```

`FERRUM_GATEWAY_FEATURES` always implies `--no-default-features`.

## Docker build args (`deploy/Dockerfile` and `deploy/Dockerfile.gateway`)

| ARG | Meaning |
|-----|---------|
| `FERRUM_VARIANT` | `full` (default), `edge`, `edge-infra` |
| `FERRUM_GATEWAY_FEATURES` | Optional cargo feature list; overrides `FERRUM_VARIANT` |
| `TARGETARCH` | `amd64` / `arm64` (BuildKit). Selects musl target in `deploy/Dockerfile`. |
| `FERRUM_GIT_SHA` | Recorded as `FERRUM_BUILD__GIT_SHA` |
| `FERRUM_BUILD_PROFILE` | Recorded as `FERRUM_BUILD__PROFILE` |

## Lab Kit mapping

| Profile / selection | Variant |
|---------------------|---------|
| `field-edge`, `beacon-only`, no WES/TES/TRS | `edge` |
| `field-edge+infra` (and other co-deploy without WES/TES/TRS) | `edge-infra` |
| `institute`, `drs-wes`, GDI, any WES/TES/TRS | `full` |

## Which variant is the external-auth gateway

`edge-infra` is the image for a gateway with external auth (ga4gh-infra clearinghouse and service registry), SQLite, DRS, Beacon, and htsget compiled in. It does not compile the WES, TES, or TRS crates. WES/TES HelixTest stubs are process env (`FERRUM_SERVICES__ENABLE_WES` and the matching TES flag), not a fourth image. `edge` is the same SQLite surfaces without the `external-auth` feature. `full` is Postgres plus WES, TES, TRS, and built-in passports, and its default feature set does not pass `--features external-auth`.

GHCR publish does not set a multi-arch platform list. Published images are linux/amd64. This pass does not add arm64.

## Health, readiness, metrics

`GET /health` returns `status` `ok` or `degraded`. Degraded means low disk space on the configured data path or clock skew past the NTP threshold. `GET /ready` returns `status` `ready` and does not check the database. This pass does not make `/ready` stricter.

There is no Prometheus scrape route. WES run metrics (`GET /ga4gh/wes/v1/runs/{id}/metrics`) are per-run JSON on the full image. They are not a node exporter.
