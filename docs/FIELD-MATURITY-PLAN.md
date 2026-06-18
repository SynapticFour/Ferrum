# Ferrum Field Edge — maturity plan

Roadmap for resource-constrained, intermittently connected field genomics (Raspberry Pi / ARM edge nodes). Tracks gaps from the Edge mode analysis and maps them to **phases** so nothing is lost between releases.

**Current tier:** **T6 operations** (backup + integrity + power E2E); **Phase 7** (ecosystem) is next.

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md), [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md), [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md), [FIELD-ONT-BASECALLING.md](FIELD-ONT-BASECALLING.md), [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md), [FIELD-OPS.md](FIELD-OPS.md), [FIELD-REGULATORY.md](FIELD-REGULATORY.md), [profiles/meta/README.md](../profiles/meta/README.md), [DECISIONS.md](../DECISIONS.md) (ADR-018–023).

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
| **T6** | Operations | Backup/restore, integrity verify, solar power E2E, Pi deploy hygiene |

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

## Phase 4 — Sync & federation (T4) — **complete**

**Goal:** When connectivity returns, data joins the larger network safely.

| # | Gap | Deliverable | Status |
|---|-----|-------------|--------|
| 4.1 | `sync_queue` SQLite migration | Embed + core migration; `ferrum-core::sync_queue` | Done |
| 4.2 | `ferrum sync enqueue` | CLI + `GET/POST /api/v1/sync/*` | Done |
| 4.3 | `ferrum sync push --target` | Hub multipart + chunked resume; residency audit | Done |
| 4.4 | Selective sync (consent/DUO filter) | `[sync] allowed_duo_codes` / `require_metadata_ref` | Done |
| 4.5 | Hub conflict policy | 409 handling; [FIELD-SYNC-HUB.md](FIELD-SYNC-HUB.md) | Done |
| 4.6 | Sneakernet export bundle | `ferrum sync export` tar.gz | Done |
| 4.7 | Federated hub registration | `[sync] register_on_push` + ga4gh-infra registry | Done |
| 4.8 | Beacon federation smoke on Edge | `ci-field-sync-e2e.sh` two-edge federation query | Done |
| 4.9 | GISAID / outbreak package on sync | `--policy` on export; ties to `ferrum outbreak package` | Done |

**Phase 4 test gate (passed):** `cargo test -p ferrum-core sync_queue`, `ci-field-sync-e2e.sh`, edge E2E.

---

## Phase 5 — Analysis pipeline (T5) — **complete**

**Goal:** Close the loop from MinION run to Beacon query in the field or via hub.

| # | Gap | Deliverable | Status |
|---|-----|-------------|--------|
| 5.1 | External Dorado/Guppy integration doc | [FIELD-ONT-BASECALLING.md](FIELD-ONT-BASECALLING.md) + ont-metrics callback | Done |
| 5.2 | Lightweight QC on Edge | `ferrum pipeline qc` (NanoStat or stub) | Done |
| 5.3 | Variant calling strategy | ADR-022 hub WES forward vs local | Done |
| 5.4 | Beacon indexing pipeline | Auto + `ferrum pipeline index-beacon`; [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md) | Done |
| 5.5 | htsget index automation | Post-ingest `htsget_index_status=ready` hook | Done |
| 5.6 | Reference genome field bundle | `profiles/references/field-bundle` + `ferrum reference install-field-bundle` | Done |

**Phase 5 test gate (passed):** `cargo test -p ferrum-core pipeline`, `ci-field-pipeline-e2e.sh`.

---

## Phase 6 — Operations & resilience — **complete**

| # | Gap | Deliverable | Status |
|---|-----|-------------|--------|
| 6.1 | Power / solar mode HTTP E2E | `FERRUM_POWER_MODE=emergency` → 503; `ci-field-ops-e2e.sh` | Done |
| 6.2 | SQLite backup CLI | `ferrum backup create\|restore\|verify` | Done |
| 6.3 | Corruption detection | `[ops] verify_checksums_on_startup` + CLI verify | Done |
| 6.4 | Log rotation on Pi | `deploy/systemd/` unit + logrotate; [FIELD-OPS.md](FIELD-OPS.md) | Done |
| 6.5 | ARM binary size budget | CI fails if release-edge ≥ 50 MB | Done |
| 6.6 | Crypt4GH Pi throughput gate | ARM64 bench smoke + documented Pi 5 target | Done |
| 6.7 | Regulatory field guide | [FIELD-REGULATORY.md](FIELD-REGULATORY.md) | Done |

**Phase 6 test gate (passed):** `cargo test -p ferrum-core ops`, `ci-field-ops-e2e.sh`.

---

## Phase 7 — Ecosystem alignment — **next**

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
bash deploy/scripts/ci-field-sync-e2e.sh
bash deploy/scripts/ci-field-pipeline-e2e.sh
bash deploy/scripts/ci-field-ops-e2e.sh
make test-demo   # full Docker stack unchanged
```

Optional: `helixtest --mode ferrum-africa --africa-profile ont,offline,federation`

---

## How to use this document

1. Pick the **lowest incomplete phase** for your sprint (**Phase 7** is next).
2. Mark items done in CHANGELOG + this file (or link PR).
3. Re-assess tier (T0–T6) after each phase for stakeholder updates.

Last updated: 2026-06-19 (Phase 6 complete).
