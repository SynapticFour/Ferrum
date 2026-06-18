# Field Edge — ecosystem alignment (Phase 7)

How Ferrum **Edge mode** maps to sibling SynapticFour repos. Use this when wiring Lab Kit, GA4GH Demo, or HelixTest for Pi / field deployments.

Related: [ECOSYSTEM.md](ECOSYSTEM.md), [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-MATURITY-PLAN.md](FIELD-MATURITY-PLAN.md).

## Naming (7.1 — Ferrum-Lab-Kit)

| Legacy / external | Ferrum canonical | Notes |
|-------------------|------------------|-------|
| `laptop` profile | **`field-edge`** / **Edge mode** | Lab Kit `config/profiles/field-edge.toml` |
| `laptop+infra` | **`field-edge+infra`** | Co-deploy ga4gh-infra on 8180–8190 |
| `--offline` CLI | **`--edge`** | Both accepted; prefer `--edge` |
| `release-laptop` | **`release-edge`** | Deprecated profile name |
| `make laptop` | **`make edge`** | Makefile alias warns |

Lab Kit install:

```bash
# From Ferrum repo (canonical installer)
./scripts/install-field-edge.sh [--with-infra]

# From Ferrum-Lab-Kit (delegates to Ferrum)
./install-edge.sh --profile field-edge
```

Ferrum owns GA4GH semantics; Lab Kit only selects backends and calls [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md) APIs.

## Raspberry Pi scenario (7.2 — Ferrum-GA4GH-Demo)

**Ferrum-GA4GH-Demo** targets Docker + GIAB benchmark on x86 servers. For **ARM field nodes (Pi 5)**, use Ferrum Edge directly — do not run the full Demo stack on Pi.

| Goal | Path |
|------|------|
| Single-binary Edge on Pi | `./scripts/install-field-edge.sh` then `ferrum demo start --edge` |
| Native ARM build | `./scripts/build-edge-native.sh --profile release-edge` |
| Co-deploy identity on hub laptop | Lab Kit `field-edge+infra` or Demo `make up-with-infra` on x86; Pi pushes via [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md) |
| Benchmark after sync | Run Demo `./run` against **hub** Ferrum, not the Pi |

See [FIELD-GA4GH-DEMO-PI.md](FIELD-GA4GH-DEMO-PI.md) for the split architecture diagram.

## HelixTest Africa (7.3)

Standard conformance unchanged: `helixtest --all --mode ferrum` (Postgres Docker stack).

Field features are opt-in:

```bash
helixtest --all --mode ferrum-africa --africa-profile offline
helixtest --all --mode ferrum-africa --africa-profile ont
helixtest --all --mode ferrum-africa --africa-profile power    # when upstream profile exists
helixtest --all --mode ferrum-africa --africa-profile all
```

**Ferrum CI supplements** HelixTest for gaps not yet in upstream profiles:

| Gap | Ferrum test / script |
|-----|----------------------|
| WES `REFERENCE_MISMATCH` | `cargo test -p ferrum-reference --test wes_mismatch` |
| Bandwidth classes | `cargo test -p ferrum-storage --test bandwidth` |
| Power / emergency 503 | `cargo test -p ferrum-gateway --test power_limit`, `ci-field-ops-e2e.sh` |
| Residency audit chain | `cargo test -p ferrum-core --test africa_network` |

Workflow: `.github/workflows/africa-conformance.yml` + `deploy/scripts/ci-field-ecosystem-e2e.sh`.

## Website copy (7.4)

Marketing strings for synapticfour.com / ferrum-field: [FIELD-WEBSITE-COPY.md](FIELD-WEBSITE-COPY.md). CLI localisation: `FERRUM_LANG=fr|de` (see [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md#localisation)).

## Laptop alias deprecation (7.5)

**Not removed in Phase 7** — one deprecation cycle per ADR-018. Removal target: **next major release (v0.3)**. Inventory: [DEPRECATED-LAPTOP-ALIASES.md](DEPRECATED-LAPTOP-ALIASES.md).
