# Performance tuning (TB-scale and edge hardware)

Related: **[docs/STORAGE-BACKENDS.md](docs/STORAGE-BACKENDS.md)** (object stores), **[docs/INSTALLATION.md](docs/INSTALLATION.md)** (database pool env/keys), **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** (streaming, `ferrum-storage`, ARM64), **[docs/PERFORMANCE-CRYPT4GH.md](docs/PERFORMANCE-CRYPT4GH.md)** (DRS Plain vs Crypt4GH micro-benchmarks, `X-Ferrum-DRS-Stream-Path`, seeded `microbench-plain-v1`), **[docs/AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md)** (Pi 5 expectations).

## ARM64 and edge hardware (Raspberry Pi 5, Apple Silicon)

Ferrum is a **first-class ARM64** target. Build flags live in [`.cargo/config.toml`](.cargo/config.toml):

| Target | CPU baseline | SIMD / crypto |
|--------|--------------|---------------|
| `aarch64-unknown-linux-gnu` | `cortex-a76` (Pi 5) | `+neon,+aes,+sha2,+crc` |
| `aarch64-apple-darwin` | `apple-m1` | `+neon,+aes,+sha2,+sha3,+crc` |
| `x86_64-unknown-linux-gnu` | `x86-64-v3` | `+aes,+sse4.2,+avx,+avx2` |

Release profile: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`. Architecture flags apply only under `[target.*]` — **debug builds** stay fast to compile.

**Native Edge build:** `./scripts/build-edge-native.sh --install` (adds `-C target-cpu=native` on-host).

### Crypt4GH throughput (`ferrum-crypt4gh`)

- Implementation: [`crypt4gh`](https://crates.io/crates/crypt4gh) crate — **X25519 + ChaCha20-Poly1305** (not AES-GCM for payloads).
- **NEON** activates when `+neon` is set (via `.cargo/config.toml` on aarch64).
- Benchmark: `cargo bench -p ferrum-crypt4gh --bench crypt_benchmark`
- **Pi 5 target:** **>500 MB/s** encrypt for 64 KiB chunks. **<200 MB/s** → check RUSTFLAGS / CPU governor.

Verify on device:

```bash
grep -m1 Features /proc/cpuinfo | grep -o aes   # Pi 5: expect aes
cargo bench -p ferrum-crypt4gh --bench crypt_benchmark -- --noplot
```

### DRS / storage I/O

| Layer | Pattern |
|-------|---------|
| `LocalStorage::get` | `tokio::fs::File::open` → streaming `AsyncRead` (no full-file buffer) |
| DRS plaintext `/stream` | 64 KiB chunks, bounded channel to HTTP body |
| Beacon indexes | SQLite — **no mmap** flat files today |

### Binary size (ARM64 edge)

`ferrum-gateway` release target: **<50 MB** (CI hard-fail on ARM64). Smallest builds: `release-edge` via `build-edge-native.sh`.

### Results table (community — update via PR)

| Platform | Crypt4GH 64 KiB | Beacon SQLite | DRS plain stream |
|----------|-----------------|---------------|------------------|
| x86_64 server | _TBD_ | _TBD_ | _TBD_ |
| Raspberry Pi 5 | **>500 MB/s target** | **<50 ms target** | **40–80 MB/s** (microSD) |
| Apple M-series | _TBD_ | _TBD_ | _TBD_ |

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

Workspace crate `ferrum-bench` and `ferrum-crypt4gh` hold Criterion targets. Compile benchmarks without running full suites:

```bash
cargo bench -p ferrum-bench --no-run
cargo bench -p ferrum-crypt4gh --bench crypt_benchmark --no-run
```

Cross-compile benchmarks for ARM64 (CI on `main`):

```bash
cargo bench -p ferrum-crypt4gh --bench crypt_benchmark --no-run \
  --target aarch64-unknown-linux-gnu
```
