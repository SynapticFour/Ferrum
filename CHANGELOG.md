# Changelog

All notable changes to this project will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`ferrum-storage` crate** — `ObjectStorage`, `LocalStorage`, `S3Storage` (moved out of `ferrum-core`). In-memory `put_bytes` uses S3 multipart from **5 MiB** with bounded concurrency; **`S3Storage::put_file`** streams large **on-disk** uploads (**8 MiB** threshold, **64 MiB** parts, parallel parts, abort incomplete multipart on error). Optional **`opendal`** feature: `OpenDalStorage` for many backends (see [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md)).
- **PostgreSQL pool tuning** — `database.min_connections`, `acquire_timeout_secs`, `idle_timeout_secs`, `max_lifetime_secs`; default `max_connections` scales with `available_parallelism` (clamped 10–100). SQLite pools get acquire timeout.
- **DRS streaming backpressure** — Plaintext `GET …/stream` uses a **bounded channel** and read timeout; Crypt4GH path keeps bounded decrypt→HTTP channel with client send timeout.
- **Graceful shutdown** (gateway) — `503` + `Retry-After` for new DRS stream requests during drain; in-flight stream tracking; `FERRUM_DRAIN_TIMEOUT_SECS` (default 300).
- **Optional build features** — `ferrum-core/libdeflate` (re-exports `noodles_bgzf` for faster BGZF; needs system libdeflate); `ferrum-drs/bam-lazy-ingest` (`ingest::bam::scan_alignment_start_positions` via `lazy_records()`); `ferrum-beacon` feature to pull `libdeflate` through core.
- **`ferrum-bench`** — Criterion benchmarks (compile with `cargo bench -p ferrum-bench --no-run`); CI job `bench-and-features` compiles benches and optional features.
- **Docs** — [PERFORMANCE.md](PERFORMANCE.md), [docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md).
- TB-scale hardening (Lesson 3): dedicated Rayon pool for blocking POSIX filesystem I/O (`ferrum_core::io::posix`, tunable via `FERRUM_POSIX_IO_THREADS`); `LocalStorage` put/delete/exists/size and Crypt4GH `LocalKeyStore` key file reads use it instead of Tokio’s default blocking pool. TES SLURM backend logs a one-time warning when GNU libc &lt; 2.24 (slow `fork`-based process spawn on some clusters).
- Crypt4GH / hot path: **`Bytes`**-based header rewrap and related throughput-oriented refactors (see crate benchmarks).
- Initial implementation of all GA4GH services (DRS, WES, TES, TRS, Beacon v2, Passports).
- Transparent Crypt4GH encryption layer with header re-wrapping (O(1) per download).
- WES support for Nextflow, CWL, WDL, Snakemake.
- Beacon v2 with three access tiers.
- Single-command demo deployment (Docker Compose, Makefile, init script).
- Helm chart and Ansible playbooks for production and HPC.
- GitHub Actions CI (test, clippy) and release workflows (multi-arch binaries).
- Install script (`install.sh`) for macOS and Linux.
- Documentation: README, ARCHITECTURE, INSTALLATION, CRYPT4GH, GA4GH, WORKFLOWS, CONTRIBUTING, SECURITY, PERFORMANCE, STORAGE-BACKENDS, docs index.
- htsget 1.3.0 ticket/stream integration (tickets refer to DRS `/stream` URLs).

### Fixed
- htsget routing reliability: compose router/state so ticket endpoints don’t 404 with empty bodies (HelixTest htsget suite).
- CI reliability: build the gateway using an official mirror (ECR public) and retry gateway Docker builds when registries are temporarily flaky.

---

*[← Back to README](README.md)*
