# Ferrum gateway image variants

Lab Kit and operators should **select a published variant**, not compile an ad-hoc matrix of every GA4GH combination. Runtime `FERRUM_SERVICES__ENABLE_*` still hides unused routes inside a variant.

## Published tags (`ghcr.io/synapticfour/ferrum`)

| Variant | Cargo | Surfaces compiled in | GHCR tags |
|---------|--------|----------------------|-----------|
| **full** | default (`full`) | Beacon, DRS, htsget, WES, TES, TRS, Passports, discovery, `/admin`, … | `:<sha>`, `:<sha>-full`, `:latest`, `:full`, version tags |
| **edge** | `--no-default-features --features edge` | DRS + Beacon + htsget + Crypt4GH on SQLite | `:<sha>-edge`, `:edge` |
| **edge-infra** | `--no-default-features --features edge,external-auth` | edge + ga4gh-infra clearinghouse + service-registry | `:<sha>-edge-infra`, `:edge-infra` |
| **full-infra** | default `full` + `--features external-auth` | full surfaces **plus** broker Passport clearinghouse / service-registry hooks | `:<sha>-full-infra`, `:full-infra`, `:<tag>-full-infra` |

`:<sha>` without a suffix is always **full** (backward compatible).

**full-infra** is the published image that both validates ga4gh-infra Passports and compiles WES/TES/TRS/`/admin` (including `GET /admin/config` for ferrum-ui). Optional `tes-docker` remains a cargo feature override via `FERRUM_GATEWAY_FEATURES`, not a fifth published variant.

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
| `FERRUM_VARIANT` | `full` (default), `edge`, `edge-infra`, `full-infra` |
| `FERRUM_GATEWAY_FEATURES` | Optional cargo feature list; overrides `FERRUM_VARIANT` |
| `TARGETARCH` | `amd64` / `arm64` (BuildKit). Selects musl target in `deploy/Dockerfile`. |
| `FERRUM_GIT_SHA` | Recorded as `FERRUM_BUILD__GIT_SHA` |
| `FERRUM_BUILD_PROFILE` | Recorded as `FERRUM_BUILD__PROFILE` |

## Lab Kit mapping

| Profile / selection | Variant |
|---------------------|---------|
| `field-edge`, `beacon-only`, no WES/TES/TRS | `edge` |
| `field-edge+infra` (and other co-deploy without WES/TES/TRS) | `edge-infra` |
| `institute`, `drs-wes`, GDI, any WES/TES/TRS (built-in passports) | `full` |
| institute + ga4gh-infra broker Passports (WES/TES/TRS/admin) | `full-infra` |

## Which variant is the external-auth gateway

`edge-infra` is the SQLite image for external auth (ga4gh-infra clearinghouse and service registry) with DRS, Beacon, and htsget. It does not compile WES, TES, TRS, or `/admin`. `full-infra` is the Postgres image that adds those full surfaces **and** `external-auth`. `full` is the same full surfaces without `external-auth`. `edge` is SQLite without `external-auth`.

WES/TES HelixTest stubs are process env (`FERRUM_WES_HELIXTEST_STUBS` / `FERRUM_TES_HELIXTEST_STUB`), not an image variant. When stubs are on, service-info must label itself **simulated** (see ADR-033).

GHCR publish does not set a multi-arch platform list. Published images are linux/amd64. This pass does not add arm64.

## GHCR matrix / runner minutes

Tag pushes build `full`, `edge`, `edge-infra`, and `full-infra` (four gateway jobs) plus `ferrum-ui`. `workflow_dispatch` defaults to `["full","edge","edge-infra"]`; pass `variants: '["full-infra"]'` to build only that gateway (plus UI). Rough wall time per gateway job on `ubuntu-latest` with GHA cache warm: **15–25 minutes**; cold: **30–45 minutes**. Four-variant tag run ≈ **60–100 runner-minutes** for gateways + ~10–20 for UI (parallel matrix).

## Health, readiness, metrics

`GET /health` returns `status` `ok` or `degraded`. Degraded means low disk space on the configured data path or clock skew past the NTP threshold. `GET /ready` returns `status` `ready` and does not check the database. This pass does not make `/ready` stricter.

There is no Prometheus scrape route. WES run metrics (`GET /ga4gh/wes/v1/runs/{id}/metrics`) are per-run JSON on the full image. They are not a node exporter.
