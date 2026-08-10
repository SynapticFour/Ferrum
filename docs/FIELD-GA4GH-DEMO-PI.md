# Ferrum Edge on Raspberry Pi vs Ferrum-GA4GH-Demo

**Ferrum-GA4GH-Demo** runs a reproducible GIAB benchmark on Docker (Postgres, MinIO, WES). **Raspberry Pi 5 field nodes** run **Ferrum Edge** (single binary, SQLite, local objects) — not the Demo compose stack.

## Recommended split

```text
┌─────────────────────┐         VSAT / sneakernet          ┌──────────────────────┐
│  Pi 5 (field Edge)  │  ─── ferrum sync push / export ──► │  Hub (Demo or prod)  │
│  install-field-edge │                                    │  GIAB benchmark WES  │
│  MinION ingest      │                                    │  HelixTest ferrum    │
└─────────────────────┘                                    └──────────────────────┘
```

## Pi field node (Edge)

```bash
git clone https://github.com/SynapticFour/Ferrum.git
cd Ferrum
./scripts/install-field-edge.sh
export PATH="$HOME/.ferrum/bin:$PATH"
ferrum demo start --edge
```

Optional co-deploy **ga4gh-infra** on a **hub** machine (not Pi): see [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md).

## When to use GA4GH-Demo

| Scenario | Use |
|----------|-----|
| Reproducible GIAB throughput benchmark | `Ferrum-GA4GH-Demo` `./run --with-infra` on x86 server |
| Field MinION collection + offline storage | Ferrum Edge (see [FIELD-OPS.md](FIELD-OPS.md)) |
| Conformance regression | HelixTest against Demo stack or production Postgres |

## Pi hardware checklist

See [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) — Pi 5, 8 GB+ RAM, USB SSD, ARM64 Linux. Binary budget: **&lt; 50 MB** (`release-edge`).
