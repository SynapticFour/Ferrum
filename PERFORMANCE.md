# Performance tuning (TB-scale)

Related: **[docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md)** (object stores), **[docs/INSTALLATION.md](docs/INSTALLATION.md)** (database pool env/keys), **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** (streaming, `ferrum-storage`), **[docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md)** (DRS Plain vs Crypt4GH micro-benchmarks, `X-Ferrum-DRS-Stream-Path`, seeded `microbench-plain-v1`).

## PostgreSQL pool

Production deployments should set `[database]` pool fields explicitly under load (see INSTALLATION configuration reference). Defaults scale `max_connections` with CPU count and set acquire/idle/lifetime timeouts to avoid unbounded waits and stale connections.

## Gateway graceful shutdown

Long DRS streams: set **`FERRUM_DRAIN_TIMEOUT_SECS`** if you need more than 5 minutes to finish in-flight streams after shutdown (Kubernetes preemption, maintenance).

## BGZF / libdeflate

Ferrum can link **libdeflate** for faster DEFLATE used by BGZF blocks (BAM, BCF, tabix-backed VCF).

- **Enable:** `cargo build -p ferrum-core --features libdeflate` (or enable the `libdeflate` feature on any crate that depends on `ferrum-core` with `ferrum-core/libdeflate`).
- **Re-export:** With the feature on, `ferrum_core` re-exports `noodles_bgzf` for downstream parsers.
- **System deps:** `libdeflate` C library — e.g. Ubuntu `apt install libdeflate-dev`, Alpine `apk add libdeflate-dev`.

Without the feature, Rust/miniz-style paths remain available via other crates; this flag is for **maximum BGZF throughput** where linking C is acceptable.

## OpenDAL storage

For many object-store backends behind one API, build `ferrum-storage` with `--features opendal` and use `OpenDalStorage`. See [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md).

## BAM lazy ingest (DRS)

`ferrum-drs` feature `bam-lazy-ingest` exposes `ingest::bam::scan_alignment_start_positions` using noodles `lazy_records()`. Use when you only need coarse positions; use full `records()` for strict validation.

## Benchmarks

Workspace crate `ferrum-bench` holds Criterion targets. Compile benchmarks without running full suites:

```bash
cargo bench -p ferrum-bench --no-run
```
