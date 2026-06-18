# Ferrum Field Edge — maturity plan

Roadmap for resource-constrained, intermittently connected field genomics (Raspberry Pi / ARM edge nodes). Tracks gaps from the Edge mode analysis and maps them to **phases** so nothing is lost between releases.

**Current tier:** **T1 complete**, moving into **T2** (identity co-deploy exists; operability hardened in Phase 1).

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md), [DECISIONS.md](../DECISIONS.md) (ADR-018, ADR-019).

---

## Maturity tiers (summary)

| Tier | Label | Operator can… |
|------|-------|----------------|
| **T0** | Demo | Run `ferrum demo start --edge`, seed data, CI E2E |
| **T1** | Ingest & store | ONT ingest (streaming), DRS, Beacon, Crypt4GH, chunked upload, disk health on Pi |
| **T2** | Identity | Co-deploy ga4gh-infra, outbreak mode, residency audit |
| **T3** | Metadata | Validate ferrum-meta at ingest; attach to DRS |
| **T4** | Sync | Queue + push objects/metadata when link returns |
| **T5** | Pipeline | QC/variant calling orchestration (hub or lightweight local) |

---

## Phase 0 — Foundation (complete)

| Item | Status | Notes |
|------|--------|-------|
| Rename Laptop → **Edge mode** (ADR-018) | Done | `--features edge`, `release-edge`, deprecated aliases |
| Field sync queue design (ADR-019) | Done | [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md); CLI stub |
| Streaming ONT ingest (no full-file RAM buffer) | Done | Multipart → temp file → `put_file` |
| ferrum-meta Phase 1 validator in CLI | Done | `ferrum meta validate` via `ferrum-meta-connect` |
| Unified field installer | Done | `scripts/install-field-edge.sh` |
| Edge E2E CI | Done | `ci-edge-demo-e2e.sh`, `test-edge-mode` job |

---

## Phase 1 — Edge operability (complete)

**Goal:** Reliable daily use on Pi 5 + USB SSD without Docker.

| # | Gap | Deliverable | Status |
|---|-----|-------------|--------|
| 1.1 | USB / external storage | `GET /health` disk stats; `[africa] objects_path` docs | Done |
| 1.2 | Watch-folder MinKNOW ingest | `ferrum ingest watch <dir>` | Done |
| 1.3 | Transfer queue wired in Edge gateway | Edge tests + gateway state (bandwidth + queue + audit) | Done |
| 1.4 | Chunked/resume upload in Edge HTTP E2E | `ci-edge-demo-e2e.sh` two-chunk `/upload/chunk` | Done |
| 1.5 | `release-edge-perf` profile | `opt-level = 3` profile in workspace `Cargo.toml` | Done |
| 1.6 | libdeflate on edge cross-build | Documented in AFRICA-DEPLOYMENT; CI checks `libdeflate` feature | Done |
| 1.7 | Offline operator doc bundle | `scripts/build-edge-doc-bundle.sh` | Done |
| 1.8 | Signed single-binary updates | `ferrum update install\|pack` (manifest + sha256) | Done |

**Phase 1 test gate (passed):** `cargo fmt`, `clippy`, `cargo test --workspace`, `ci-edge-demo-e2e.sh`.

---

## Phase 2 — Metadata & provenance (T3) — **next**

**Goal:** Field collection metadata is first-class, not an afterthought.

| # | Gap | Deliverable | Tests |
|---|-----|-------------|-------|
| 2.1 | ferrum-meta ↔ DRS binding | Store `metadata_ref` on `drs_objects`; ingest API accepts bundle | Rust + HelixTest |
| 2.2 | Interactive metadata CLI wizard | `ferrum meta init --profile pathogen\|h3africa` | CLI golden |
| 2.3 | Expand validator to pathogen + H3Africa profiles | Extend `ferrum-meta-connect` | Fixtures from ferrum-meta repo |
| 2.4 | LinkML parity or JSON Schema generation | CI sync schema from ferrum-meta releases | Cross-repo workflow |
| 2.5 | Provenance on Edge | Capture collector, GPS, timestamp at ingest | Residency audit entries |
| 2.6 | Consent / DUO at collection | Passport visa + ferrum-meta `data_use_conditions` | Pilot with Pasteur Tunis |
| 2.7 | Paper → digital | Import YAML/CSV from structured forms | Operator doc |

---

## Phase 3 — Auth & long offline (T2 hardened)

**Goal:** Days without internet; multiple operators; auditable access.

| # | Gap | Deliverable | Tests |
|---|-----|-------------|-------|
| 3.1 | Single installer polish | `install-field-edge.sh` in release CI; Pi 5 smoke | ARM runner |
| 3.2 | JWKS long TTL playbook | Default 7d cache; doc rotation without network | ga4gh-infra + Ferrum |
| 3.3 | Embedded IdP visa issuance | Field roles (collector, analyst, sync operator) | E2E co-deploy |
| 3.4 | Shared device / multi-user | Local account model on Edge | Security review |
| 3.5 | Key rotation offline | Pre-provisioned key sets in signed bundle | ADR |
| 3.6 | NTP / clock integrity | Warn if skew > threshold; audit chain note | Health endpoint |

---

## Phase 4 — Sync & federation (T4)

**Goal:** When connectivity returns, data joins the larger network safely.

| # | Gap | Deliverable | Tests |
|---|-----|-------------|-------|
| 4.1 | `sync_queue` SQLite migration | Embed migration + repo | Unit |
| 4.2 | `ferrum sync enqueue` | CLI + API optional | Integration |
| 4.3 | `ferrum sync push --target` | Hub adapter: DRS multipart + Crypt4GH stream | Bandwidth/resume tests |
| 4.4 | Selective sync (consent/DUO filter) | Policy before enqueue | Compliance review |
| 4.5 | Hub conflict policy | 409 + operator message | Doc + mock hub |
| 4.6 | Sneakernet export bundle | Tarball: objects + meta + audit slice | CLI |
| 4.7 | Federated hub registration | Auto-register in ga4gh-infra registry when online | Discovery tests |
| 4.8 | Beacon federation smoke on Edge | Two-edge CI job | HelixTest federation profile |
| 4.9 | GISAID / outbreak package on sync | Tie to existing `ferrum outbreak package` | Existing Rust tests |

See [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md) for queue schema and CLI contract.

---

## Phase 5 — Analysis pipeline (T5)

**Goal:** Close the loop from MinION run to Beacon query in the field or via hub.

| # | Gap | Deliverable | Tests |
|---|-----|-------------|-------|
| 5.1 | External Dorado/Guppy integration doc | Runbook + `ont-metrics` callback | Manual |
| 5.2 | Lightweight QC on Edge | Optional subprocess NanoStat → metrics API | WES optional / CLI |
| 5.3 | Variant calling strategy | Hub WES forward vs local minimap2/pipeline | ADR |
| 5.4 | Beacon indexing pipeline | VCF → SQLite index job; document limits | Benchmark |
| 5.5 | htsget index automation | Post-ingest index hook | Integration |
| 5.6 | Reference genome field bundle | Pre-seeded refs for pathogen + GRCh38 | Reference registry tests |

---

## Phase 6 — Operations & resilience

| # | Gap | Deliverable | Tests |
|---|-----|-------------|-------|
| 6.1 | Power / solar mode HTTP E2E | HelixTest africa power profile | HelixTest |
| 6.2 | SQLite backup CLI | `ferrum backup create\|restore` | Round-trip |
| 6.3 | Corruption detection | Checksum verify on startup option | Unit |
| 6.4 | Log rotation on Pi | systemd unit + doc | Deploy |
| 6.5 | ARM binary size budget | Keep < 50 MB; track in CI | build-arm64 job |
| 6.6 | Crypt4GH Pi throughput gate | >500 MB/s on Pi 5 in release notes | bench-arm64 |
| 6.7 | Regulatory field guide | GDPR + local health law pointers | Doc |

---

## Phase 7 — Ecosystem alignment

| # | Gap | Deliverable |
|---|-----|-------------|
| 7.1 | Ferrum-Lab-Kit `field-edge` naming | Align docs with Edge mode |
| 7.2 | Ferrum-GA4GH-Demo Pi scenario | Point to `install-field-edge.sh` |
| 7.3 | HelixTest `ferrum-africa` expansion | WES ref mismatch, bandwidth, power |
| 7.4 | Website / i18n | synapticfour.com ferrum-field copy |
| 7.5 | Remove deprecated `laptop` aliases | Major release after one deprecation cycle |

---

## Test gate (every phase)

Before merging each phase:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test -p ferrum-embed
cargo test -p ferrum-meta-connect
./scripts/build-edge-native.sh --no-native-cpu
bash deploy/scripts/ci-edge-demo-e2e.sh
make test-demo   # full Docker stack unchanged
```

Optional: `helixtest --mode ferrum-africa --africa-profile ont,offline,federation`

---

## How to use this document

1. Pick the **lowest incomplete phase** for your sprint (**Phase 2** is next).
2. Mark items done in CHANGELOG + this file (or link PR).
3. Re-assess tier (T0–T5) after each phase for stakeholder updates.

Last updated: 2026-06-18 (Phase 1 complete).
