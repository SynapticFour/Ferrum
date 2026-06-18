# Ferrum Field Edge — maturity plan

Roadmap for resource-constrained, intermittently connected field genomics (Raspberry Pi / ARM edge nodes). Tracks gaps from the Edge mode analysis and maps them to **phases** so nothing is lost between releases.

**Current tier:** **T2 hardened** (auth + long offline); **Phase 4** (sync) is next.

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md), [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md), [profiles/meta/README.md](../profiles/meta/README.md), [DECISIONS.md](../DECISIONS.md) (ADR-018–020).

---

## Maturity tiers (summary)

| Tier | Label | Operator can… |
|------|-------|----------------|
| **T0** | Demo | Run `ferrum demo start --edge`, seed data, CI E2E |
| **T1** | Ingest & store | ONT ingest (streaming), DRS, Beacon, Crypt4GH, chunked upload, disk health on Pi |
| **T2** | Identity | Co-deploy ga4gh-infra, field roles, JWKS offline, shared device accounts, clock integrity |
| **T3** | Metadata | Validate ferrum-meta at ingest; attach to DRS via `metadata_ref`; field provenance |
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

All items 1.1–1.8 done. See prior release notes.

---

## Phase 2 — Metadata & provenance (T3) (complete)

**Goal:** Field collection metadata is first-class, not an afterthought.

All items 2.1–2.7 done. See prior release notes.

---

## Phase 3 — Auth & long offline (T2 hardened) — **complete**

**Goal:** Days without internet; multiple operators; auditable access.

| # | Gap | Deliverable | Status |
|---|-----|-------------|--------|
| 3.1 | Single installer polish | `install-field-edge.sh` in release CI + `ci-field-edge-install-smoke.sh` | Done |
| 3.2 | JWKS long TTL playbook | Default 7d cache; `jwks_file` offline; [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md) | Done |
| 3.3 | Embedded IdP visa issuance | Field roles + ingest enforcement; ga4gh-infra co-deploy doc | Done |
| 3.4 | Shared device / multi-user | `edge_operator_accounts` + `ferrum auth account/login` | Done |
| 3.5 | Key rotation offline | JWKS in update bundle; ADR-020 | Done |
| 3.6 | NTP / clock integrity | `clock` on `/health`; degraded on skew | Done |

**Phase 3 test gate (passed):** `cargo test -p ferrum-core`, `ci-field-edge-install-smoke.sh`, edge E2E.

---

## Phase 4 — Sync & federation (T4) — **next**

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
cargo test -p ferrum-drs --test metadata_ref
./scripts/build-edge-native.sh --no-native-cpu
bash deploy/scripts/ci-edge-demo-e2e.sh
bash deploy/scripts/ci-field-edge-install-smoke.sh
make test-demo   # full Docker stack unchanged
```

Optional: `helixtest --mode ferrum-africa --africa-profile ont,offline,federation`

---

## How to use this document

1. Pick the **lowest incomplete phase** for your sprint (**Phase 4** is next).
2. Mark items done in CHANGELOG + this file (or link PR).
3. Re-assess tier (T0–T5) after each phase for stakeholder updates.

Last updated: 2026-06-19 (Phase 3 complete).
