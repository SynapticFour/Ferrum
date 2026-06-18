# Field & offline deployment guide

> **Website:** [synapticfour.com/en/ferrum-field](https://synapticfour.com/en/ferrum-field) — public overview for resource-constrained and field-lab deployments.

## Offline-First and Edge mode

Genomics labs in resource-constrained settings often operate on shared laptops (16 GB RAM, spinning disks, Ubuntu 22.04), with intermittent or no internet, and without access to container registries or cloud object storage. Ferrum **Edge mode** addresses this with:

- **SQLite** instead of PostgreSQL (single file under `~/.ferrum/ferrum.db`)
- **Local filesystem** object storage instead of MinIO/S3 (`~/.ferrum/objects/`)
- **Non-fatal startup** when auth endpoints or the network are unavailable
- **`FERRUM_OFFLINE=1`** shortcut (no config file edit required)

### One command to start (recommended)

After installing the `ferrum` binary (see [INSTALLATION.md](INSTALLATION.md)):

```bash
export PATH="$HOME/.ferrum/bin:$PATH"
ferrum demo start --edge
```

That single command starts **ferrum-gateway** in Edge mode with SQLite + local storage. No Docker, PostgreSQL, or MinIO required.

Alternative equivalents:

```bash
FERRUM_OFFLINE=1 ferrum start          # gateway only (same backends, fewer console hints)
FERRUM_OFFLINE=1 ferrum-gateway        # if you invoke the binary directly
```

Expected console output:

```
[ferrum] Starting in Edge mode (SQLite + local storage).
[ferrum] Data will be stored at ~/.ferrum/
[ferrum] To use production backends, set FERRUM_CONFIG=/path/to/config.toml
```

Verify:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/ga4gh/drs/v1/service-info
# Full offline round-trip (ingest → stream), same as CI:
sh deploy/scripts/ci-laptop-demo-e2e.sh
```

### What Edge mode includes (and excludes)

Edge mode is a **single-process, embedded stack** — not the full Docker demo.

| Available | Endpoint | Backend |
|-----------|----------|---------|
| Health | `/health` | — |
| DRS (ingest, metadata, stream) | `/ga4gh/drs/v1` | SQLite + local files |
| Beacon v2 | `/ga4gh/beacon/v2` | SQLite |
| htsget (when objects are indexed) | `/ga4gh/htsget/v1` | Same SQLite as DRS |

| Not available (requires PostgreSQL) | Typical response |
|-------------------------------------|------------------|
| WES, TES, TRS | `503` / service disabled |
| Passports / OIDC flows | `503` or skipped at startup |
| Full HelixTest matrix | Use Docker demo or production stack |

For conformance testing and the complete GA4GH surface, use `deploy/docker-compose.yml` or a Postgres-backed deployment — see [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md).

### Auth & long offline (Phase 3)

Co-deploy **ga4gh-infra** for Passport issuance (`scripts/install-field-edge.sh`), or use **local Edge accounts** on shared devices:

```bash
ferrum auth account add --username alice --role collector --pin '****'
ferrum auth login --username alice --pin '****'   # Bearer token for ingest
```

Configure offline JWKS (`jwks_file`, 7-day cache), enforce roles with `require_auth = true`, and monitor clock skew on `/health`. Full playbook: [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md).

### Resource requirements (realistic expectations)

These numbers set expectations for **Edge mode only** (one `ferrum-gateway` process, no Docker stack). They combine design targets from the Africa deep-dive, preflight checks, and typical Rust + SQLite behaviour. **Ferrum does not yet publish formal load-test benchmarks for Edge mode**; treat disk and RAM for *your data* as the main scaling factor.

#### By deployment profile

| Profile | RAM | Free disk | CPU | Typical use |
|---------|-----|-----------|-----|-------------|
| **Minimum** | 4 GB | 10 GB | 2 cores | Evaluation, small test files, single user |
| **Recommended** | 8–16 GB | 50 GB+ | 4 cores | Shared lab laptop, modest VCF/BAM working sets |
| **Heavy data** | 16–32 GB | 100 GB – 1 TB+ | 4+ cores | Larger objects; disk dominates, not the binary |

**Disk:** Ferrum itself is small (binary + SQLite metadata). Genomic objects live under `~/.ferrum/objects/` and grow with your ingest — plan disk from your dataset size, not from Ferrum’s install footprint.

**RAM:** Idle gateway typically uses on the order of **~100–250 MB RSS** (platform-dependent; not CI-gated). Optional cap via `[africa] max_memory_mb` (see below). Leave headroom for OS, browser, and analysis tools on shared laptops.

**CPU:** SQLite allows one writer at a time; Edge mode suits **single-user or low-concurrency** lab workflows, not multi-tenant throughput.

**Network:** Not required at **runtime** once the binary is installed. Internet may be needed once for `install.sh` or `cargo build --release`.

#### By platform

| Platform | Edge mode | Install path | Notes |
|----------|-------------|--------------|-------|
| **Linux x86_64** | Supported | `install.sh`, musl release binary | Primary target; memory cap monitoring via `/proc/self/status` |
| **Linux ARM64** (Raspberry Pi 4/5, ARM SBCs) | Supported | `install.sh` (`aarch64-unknown-linux-musl`) | **4 GB RAM is tight** — 8 GB recommended; prefer USB SSD over SD card for SQLite + objects |
| **macOS** (Intel / Apple Silicon) | Supported | `install.sh`, darwin release binary | Memory cap logs are best-effort (no Linux `/proc`); otherwise same as Linux |
| **Windows (native)** | Not supported | — | No official Windows binary or install script |
| **Windows (WSL2 Ubuntu)** | Supported | Linux flow inside WSL | Treat as Linux; store data on Linux filesystem (`~/.ferrum/`), not `/mnt/c/` for performance |

#### Preflight check

Before rollout on a target machine:

```bash
./scripts/deployment_preflight.sh --scenario edge
```

Use `--scenario offline` for the **air-gapped Docker bundle** path (heavier: expects Docker and 16 GB RAM). Use `--scenario edge` for **embedded Edge mode** without containers.

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
| **Laptop / offline** | `ferrum demo start --edge`, `FERRUM_OFFLINE=1`, `[africa] offline_first`, or default sqlite driver without Postgres URL | SQLite | Local path |

Production PostgreSQL and S3 code paths are unchanged. HelixTest conformance continues to run against the full Postgres stack.

### Performance on Raspberry Pi 5

Raspberry Pi 5 (Cortex-A76, ARM64) is the primary **edge hardware** target for Africa field deployments.

| Workload | Expected performance | Limiting factor |
|----------|---------------------|-----------------|
| **Crypt4GH encrypt** | **>500 MB/s** (64 KiB chunks, release + NEON) | CPU; verify with `cargo bench -p ferrum-crypt4gh` |
| **Beacon v2 query** (local SQLite) | **<50 ms** typical | SQLite + disk; use USB SSD not microSD for indexes |
| **DRS download** (plain `/stream`) | **~40–80 MB/s** on good microSD; **100+ MB/s** on USB SSD | Storage I/O, not CPU |
| **Idle RAM** | ~100–250 MB RSS | Set `[africa] max_memory_mb` to leave headroom on 4 GB models |

**USB / external storage:** Point `[africa] objects_path` (or `storage.base_path`) at a mounted USB SSD — e.g. `/mnt/ferrum-data/objects`. Ferrum reports free space on that path in `GET /health` (`disk.free_bytes`, `disk.warn_low_space` when below 10%).

**Performance build:** Use profile `release-edge-perf` (`opt-level = 3`) when CPU throughput matters more than binary size:

```bash
CARGO_PROFILE=release-edge-perf ./scripts/build-edge-native.sh --install
```

**Optional BGZF/libdeflate:** For faster VCF/BGZF on edge builds, install `libdeflate-dev` and build with `cargo build -p ferrum-beacon --features libdeflate` (see [PERFORMANCE.md](../PERFORMANCE.md)).

Verify hardware crypto extensions:

```bash
grep -m1 Features /proc/cpuinfo | grep -o aes
# Expected on Pi 5: aes (ARMv8 Crypto Extensions)
```

Build with ARM optimisations (from repo root):

```bash
./scripts/build-edge-native.sh --install
# Uses .cargo/config.toml cortex-a76 + release-edge profile
```

See [PERFORMANCE.md](../PERFORMANCE.md) for benchmark methodology and CI size targets (<50 MB gateway binary).

### Native optimized build (recommended)

Ferrum ships a **single optimized binary** for Edge mode: DRS + Beacon + htsget + Crypt4GH on SQLite (no WES/TES/TRS/Postgres in the binary). GitHub Releases and `install.sh --offline` use this build.

**Simplest path** — auto-detect OS, CPU, and apply native optimizations:

```bash
./scripts/build-edge-native.sh --install
ferrum demo start --edge
```

What the script does:

| Step | Behaviour |
|------|-----------|
| OS / arch | Linux x86_64, Linux ARM64 (Raspberry Pi), macOS Intel/Apple Silicon |
| CPU | `-C target-cpu=native` when building **on** the target machine (best performance) |
| Profile | `release-edge` — LTO, strip, size-optimized (`opt-level = "s"`) |
| Features | `--no-default-features --features edge` — slim embedded stack only |

Flags:

```bash
./scripts/build-edge-native.sh --install          # build + install to ~/.ferrum/bin
./scripts/build-edge-native.sh --no-native-cpu    # portable binary (same arch, generic CPU)
./scripts/build-edge-native.sh --target aarch64-unknown-linux-gnu  # cross-compile hint
make edge                                         # same as --install from repo root
```

At startup, the gateway **auto-detects** RAM and CPU model (Linux `/proc`, macOS `sysctl`) and logs a platform summary. If `[africa] max_memory_mb` is unset, it sets a cap to **80% of detected RAM** to reduce OOM risk on shared laptops.

**Production / Docker stack** still uses the full gateway (`--features full`, default in `cargo build -p ferrum-gateway`). Laptop and full binaries share the same CLI: `ferrum demo start --edge` vs Docker demo.

### Build once, run offline (manual)

On a build machine, then copy `~/.ferrum/bin/ferrum-gateway` to the field laptop:

```bash
./scripts/build-edge-native.sh --install
# or from repo without install:
./scripts/build-edge-native.sh
# Copy target/<triple>/release-edge/ferrum-gateway to the target machine, then:
ferrum demo start --edge
```

See also [OFFLINE-AIRGAP.md](deployment/OFFLINE-AIRGAP.md) for signed bundles.

### Decision rationale (ADR summary)

Embedded backends trade horizontal scalability for operability: SQLite suits single-user laptop deployments; PostgreSQL remains the production source of truth. Local storage avoids S3 API dependencies while preserving the same `ObjectStorage` trait used by DRS ingest and streaming in production.

See also: [deployment README](deployment/README.md), [INSTALLATION.md](INSTALLATION.md).

## Nanopore Ingestion

Field labs in Africa predominantly use **Oxford Nanopore MinION** sequencers. Ferrum ingests raw ONT files (POD5/FAST5/BLOW5) or pre-basecalled FASTQ via:

```http
POST /api/v1/ingest/ont
Content-Type: multipart/form-data

ont_metadata (JSON) + file (binary) [+ optional ferrum_meta (YAML/JSON)]
```

Optional multipart field **`ferrum_meta`** attaches a validated ferrum-meta submission; the dataset alias is stored as `drs_objects.metadata_ref`. Provenance fields on `ont_metadata` (`collector`, `collected_at`, `location_label`, `latitude`, `longitude`) are recorded in the residency audit as `collection_recorded`.

**Field workflow:**

```bash
# 1. Generate collection metadata (pathogen or H3Africa profile)
ferrum meta init --profile pathogen --output ~/collection.yaml

# 2. Watch MinKNOW output and attach metadata + collector
ferrum ingest watch ~/minion_runs --meta-bundle ~/collection.yaml --collector "Dr. A"
```

See [profiles/meta/README.md](../profiles/meta/README.md) and [FIELD-MATURITY-PLAN.md](FIELD-MATURITY-PLAN.md) Phase 2.

Basecalling runs **externally** (Dorado/Guppy). Ferrum stores the canonical DRS object plus optional `ont_metrics` JSON on `drs_objects.ont_metrics`. Pathogen organism tags are written to `pathogen_annotations` for Beacon queries. ferrum-meta bundles are validated offline and linked via `metadata_ref`.

WES workflow template: [`tools/workflows/ont-qc.wdl`](../tools/workflows/ont-qc.wdl) (NanoStat/NanoPlot → metrics back via ingest).

See [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md#ont-nanopore-ingestion).

## Multi-Pathogen Surveillance

Beacon v2 accepts optional pathogen filters in `requestParameters`:

| Field | Example |
|-------|---------|
| `organism` | `Mycobacterium_tuberculosis` |
| `amrGene` | `blaNDM-1` |
| `serotype` | `O1` |
| `minQscore` | `10` |

Human genomics queries **without** these fields behave exactly as before. Meta-schema extension: [`crates/ferrum-beacon/schemas/pathogen-extension.json`](../crates/ferrum-beacon/schemas/pathogen-extension.json).

## Outbreak Mode

Policy-based emergency Beacon access for WHO/Africa CDC partners during declared outbreaks. **Disabled by default** — enable in config:

```toml
[outbreak]
enabled = true

[[outbreak.policies]]
name = "mpox_who_emergency"
trigger_pathogen = "Monkeypox_virus"
emergency_recipients = ["who.int", "africacdc.org"]
access_level = "beacon_only"
gisaid_auto_package = true
```

Activation requires Passport visa `ferrum:outbreak_activator`. Full reference: [OUTBREAK-MODE.md](OUTBREAK-MODE.md).

## Federated Beacon

P2P federated queries without a central coordinator. **Disabled by default** — requires `federate=true`:

```http
GET /ga4gh/beacon/v2/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T
```

Configure peers under `[federation]` in config. Full reference: [FEDERATION.md](FEDERATION.md).

## Bandwidth-Adaptive Transfer

DRS transfers adapt to measured link quality:

- Rolling bandwidth estimate (last 10 transfers, EMA) → `High` / `Medium` / `Low` / `VeryLow`.
- Chunk sizes: 64 MB / 16 MB / 4 MB / 512 KB.
- `transfer_checkpoints` table + `resume_token` on access responses; resume via `GET …/access?resume_token=…`.
- **VeryLow** links: large uploads queued (gateway `TransferQueue`); DRS may emit `Content-Encoding: zstd` on non-Crypt4GH streams.
- Resumable uploads: `POST /api/v1/ingest/upload/chunk` with `total_bytes`, `chunk_offset`, optional `upload_token`; chunk sizes follow `BandwidthClass`.

Thresholds configurable under `[bandwidth]` in `config.toml`.

## Solar / Battery Mode

Reads Linux `/sys/class/power_supply/` (macOS: `pmset`) when `[power] enabled = true` (default on Linux):

| Mode | Trigger | Effect |
|------|---------|--------|
| `HighPerformance` | AC power | All features |
| `LowPower` | Battery &lt; 50% | Max 4 concurrent requests; background checksum/index work paused |
| `Emergency` | Battery &lt; 10% | Refuse new connections; write `~/.ferrum/CHECKPOINT`; exit after 30s drain |

Override: `FERRUM_POWER_MODE=low_power|emergency|high_performance`.

## Data Residency Audit

Append-only SHA-256 chained log of accesses, downloads, Beacon queries, and federation fan-out.

```http
GET /api/v1/audit/residency?from=…&to=…
GET /api/v1/audit/residency/verify
```

Full reference: [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md).

## Community African Reference Genomes

Ferrum ships a **pluggable reference registry** (`reference_genomes` table) with seeded entries for GRCh38, T2T-CHM13, H3Africa_v1, AWI-GEN_panel, Pf3D7_v3, and MTB_H37Rv. FASTA files are **not bundled** — the registry records metadata and optional DRS object IDs after operator ingest.

- **List references:** `GET /api/v1/references`
- **Load FASTA:** ingest via DRS, then `PUT /api/v1/references/{id}/load`
- **WES warnings:** GRCh38 + African-origin inputs → `REFERENCE_MISMATCH` warning (non-blocking)
- **Beacon:** pathogen queries include `meta.referenceGenome` when matched

Full reference: [REFERENCE-GENOMES.md](REFERENCE-GENOMES.md).

## HelixTest Africa Mode

Opt-in conformance for Africa-specific features without changing standard `--mode ferrum`:

```bash
helixtest --all --mode ferrum-africa --africa-profile offline
helixtest --all --mode ferrum-africa --africa-profile all
```

Profiles: `offline`, `ont`, `outbreak`, `federation`, `all`. See [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md).

**What Africa HelixTest does not cover yet** (bandwidth/resume, power mode, WES reference mismatch, federation without a peer, outbreak when disabled in config): [TEST-COVERAGE-GAPS.md](TEST-COVERAGE-GAPS.md).

**Demo stack:** after upgrading Ferrum on an existing Postgres volume, `ferrum-init` applies only migrations not recorded in `_ferrum_init_migrations` (see init script). Use `docker compose down -v` only when you need a completely fresh database.

## GISAID Metadata

Outbreak Mode can auto-build GISAID submission archives when `gisaid_auto_package = true` in an outbreak policy. Capture metadata at ingest so packaging is complete:

```json
POST /api/v1/ingest/register
{
  "items": [{
    "kind": "existing_object",
    "storage_backend": "local",
    "storage_key": "drs/seq-1",
    "size": 4096,
    "gisaid_metadata": {
      "collection_date": "2025-11-01",
      "location": "Liberia/Margibi",
      "host": "Human",
      "submitting_lab": "NPHIL",
      "submitting_lab_address": "Monrovia, Liberia",
      "originating_lab": "NPHIL National Reference Laboratory"
    }
  }]
}
```

Metadata is stored on `drs_objects.gisaid_metadata`. On outbreak activation, Ferrum returns `gisaid_warnings` when tagged pathogen objects are missing required fields. Build archives with `ferrum outbreak package --policy <name>`.

## Field sync (Edge → hub)

When VSAT or a site visit restores connectivity, push queued objects to a hub Ferrum instance:

```bash
ferrum sync enqueue --all-local --target https://hub.example.org
ferrum sync status
ferrum sync push --target https://hub.example.org
```

Offline USB handoff: `ferrum sync export --output bundle.tar.gz --policy <outbreak-policy>`.

Configure consent filtering via `[sync]` in `config.toml` (`allowed_duo_codes`, `require_metadata_ref`). Hub duplicate-sample policy: [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md).

## Field analysis pipeline (Phase 5)

After ingest, Edge nodes can run lightweight QC, Beacon indexing, and optional hub WES forward:

```bash
ferrum pipeline qc --object-id <drs-id> --fastq reads.fastq --gateway http://127.0.0.1:8080
ferrum pipeline index-beacon --object-id <drs-id>
ferrum pipeline htsget-status --object-id <drs-id>
ferrum reference install-field-bundle --gateway http://127.0.0.1:8080
```

Configure post-ingest hooks via `[pipeline]` in `config.toml` (`auto_htsget_index`, `auto_index_beacon`, `default_beacon_dataset`). Dorado/Guppy runbook: [FIELD-ONT-BASECALLING.md](FIELD-ONT-BASECALLING.md). Beacon limits: [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md). Variant calling strategy: ADR-022 in [DECISIONS.md](../DECISIONS.md).

## Field operations (Phase 6)

Backup, integrity checks, and Pi deployment:

```bash
ferrum backup create --output ~/backups/ferrum-$(date +%Y%m%d).tar.gz
ferrum backup verify
# stop gateway, then:
ferrum backup restore --archive ~/backups/ferrum-20260619.tar.gz --force
```

Enable startup integrity gate: `[ops] verify_checksums_on_startup = true`. Full guide: [FIELD-OPS.md](FIELD-OPS.md). Regulatory pointers: [FIELD-REGULATORY.md](FIELD-REGULATORY.md).

## Localisation

The Ferrum CLI supports English (default), French, and German via `FERRUM_LANG`:

```bash
export FERRUM_LANG=fr   # or de
ferrum --help
ferrum demo start --edge
```

User-facing CLI messages (help text, demo start, migration status) are translated. API responses remain English unless a future locale layer is added.

## Deployment models and pricing

Ferrum is designed for **flexible deployment** rather than a single SaaS shape:

| Model | Typical use | Notes |
|-------|-------------|--------|
| **Edge mode** | Field labs, intermittent connectivity | Single binary, SQLite, local disk; no cloud dependency |
| **Institutional server** | National public-health nodes | Postgres + object storage on-prem; full GA4GH stack |
| **Federated network** | Cross-border surveillance | Beacon federation with residency audit |
| **Managed hosting** | Partners / integrators | Operator-run; pricing negotiated (support, SLA, training) |

Synaptic Four does **not** publish fixed per-country prices in this repository. Commercial terms, support tiers, and institutional agreements are described narratively in [BUSINESS-MODEL.md](BUSINESS-MODEL.md). Pilot outreach email authentication: [OPERATIONS.md](OPERATIONS.md).

---

*[← Documentation index](README.md)*
