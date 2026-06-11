# Africa Deployment Guide

## Offline-First and Laptop Mode

Genomics labs in resource-constrained settings often operate on shared laptops (16 GB RAM, spinning disks, Ubuntu 22.04), with intermittent or no internet, and without access to container registries or cloud object storage. Ferrum **Laptop Mode** addresses this with:

- **SQLite** instead of PostgreSQL (single file under `~/.ferrum/ferrum.db`)
- **Local filesystem** object storage instead of MinIO/S3 (`~/.ferrum/objects/`)
- **Non-fatal startup** when auth endpoints or the network are unavailable
- **`FERRUM_OFFLINE=1`** shortcut (no config file edit required)

### Quickstart (offline)

```bash
# Build once where network is available
cargo build --release -p ferrum-gateway

# On the laptop (no network required)
export FERRUM_OFFLINE=1
./target/release/ferrum-gateway
# or: ferrum demo start --offline   (ferrum-cli)
```

Expected console output:

```
[ferrum] PostgreSQL not detected. Starting in Laptop Mode (SQLite + local storage).
[ferrum] Data will be stored at ~/.ferrum/
[ferrum] To use production backends, set FERRUM_CONFIG=/path/to/config.toml
```

Verify:

```bash
curl http://127.0.0.1:8080/health
# Full offline round-trip (ingest → stream), same as CI:
sh deploy/scripts/ci-laptop-demo-e2e.sh
```

### Configuration

```toml
[africa]
offline_first = true
max_memory_mb = 4096          # optional RAM cap (Linux: /proc/self/status VmRSS)
sqlite_path = "~/.ferrum/ferrum.db"
objects_path = "~/.ferrum/objects/"
```

Environment overrides:

| Variable | Effect |
|---|---|
| `FERRUM_OFFLINE=1` | Force offline-first / embedded backends |
| `FERRUM_CONFIG` | Production config path (Postgres + S3) |

### Switching modes

| Mode | Trigger | Database | Storage |
|---|---|---|---|
| **Production** | `database.url = postgres://…` in config | PostgreSQL | S3 / MinIO |
| **Laptop / offline** | `FERRUM_OFFLINE=1`, `[africa] offline_first`, or default sqlite driver without Postgres URL | SQLite | Local path |

Production PostgreSQL and S3 code paths are unchanged. HelixTest conformance continues to run against the full Postgres stack.

### Decision rationale (ADR summary)

Embedded backends trade horizontal scalability for operability: SQLite suits single-user laptop deployments; PostgreSQL remains the production source of truth. Local storage avoids S3 API dependencies while preserving the same `ObjectStorage` trait used by DRS ingest and streaming in production.

See also: [deployment README](deployment/README.md), [OFFLINE-AIRGAP](deployment/OFFLINE-AIRGAP.md).
