# Field Edge — gap reassessment (Phases 0–7)

Post–Phase 7 review of the [FIELD-MATURITY-PLAN.md](FIELD-MATURITY-PLAN.md) roadmap. **No blocking gaps** remain for a minimal field deployment (MinION ingest → QC → Beacon → sync). Items below are **follow-ups**, not Phase 0–7 deliverables.

Date: 2026-06-19

---

## Phase completion summary

| Phase | Tier | Status | Notes |
|-------|------|--------|-------|
| 0 Foundation | T0 | Complete | Edge rename, installer, E2E CI |
| 1 Edge operability | T1 | Complete | Streaming ingest, disk health, chunked upload |
| 2 Metadata | T3 | Complete | ferrum-meta, metadata_ref, provenance |
| 3 Auth / offline | T2 | Complete | JWKS 7d, edge accounts, clock on /health |
| 4 Sync | T4 | Complete | Queue, push, sneakernet, federation smoke |
| 5 Pipeline | T5 | Complete | QC, Beacon index, htsget hooks, ref bundle |
| 6 Operations | T6 | Complete | Backup, integrity, power E2E, systemd, 50 MB gate |
| 7 Ecosystem | T7 | Complete | Lab-Kit/Demo docs, HelixTest supplements, i18n, laptop deprecation inventory |

---

## Resolved since initial gap analysis

| Original gap | Resolution |
|--------------|------------|
| Laptop vs Edge naming confusion | ADR-018 + Phase 7 docs; aliases documented for v0.3 removal |
| No field backup path | `ferrum backup create\|restore\|verify` (Phase 6) |
| Silent object corruption | Checksum verify + optional startup gate (Phase 6) |
| No sync when online returns | sync_queue + push (Phase 4) |
| Beacon dead-end after ingest | Auto VCF index + CLI (Phase 5) |
| Multi-operator on shared device | edge_operator_accounts (Phase 3) |
| Lab Kit / Demo Pi confusion | FIELD-ECOSYSTEM.md, FIELD-GA4GH-DEMO-PI.md (Phase 7) |

---

## Open follow-ups (non-blocking)

### High value (next releases)

| # | Area | Gap | Suggested action |
|---|------|-----|------------------|
| F1 | **HelixTest upstream** | `power`, `bandwidth/resume` profiles not in HelixTest `main` yet | Ferrum Rust + E2E cover gaps; contribute cases to HelixTest repo |
| F2 | **WES on Edge** | `ferrum pipeline forward-wes` is CLI hook only; no bundled workflows | Hub WES + ADR-022; document required hub setup |
| F3 | **Local variant caller** | ADR-022 defers minimap2/small-caller | Opt-in profile when Pi CPU budget allows |
| F4 | **Federation CI** | Second gateway for `federation` HelixTest profile | Optional CI matrix; manual `FERRUM_AFRICA_PEER_URL` |
| F5 | **Outbreak in demo** | Africa outbreak profile skips when `[outbreak] enabled = false` | CI overlay TOML for outbreak job |
| F6 | **Laptop alias removal** | v0.3 major per DEPRECATED-LAPTOP-ALIASES.md | Mechanical rename PR |

### Medium (operations)

| # | Area | Gap | Suggested action |
|---|------|-----|------------------|
| M1 | **Postgres backup** | Phase 6 backup is SQLite-only | Document pg_dump for hub; out of Edge scope |
| M2 | **Power on /health** | Power mode not exposed in JSON health | Optional `power` block on `/health` for NOC |
| M3 | **i18n coverage** | Field CLI subcommands still English-only in `--help` | Extend i18n.rs for sync/backup/pipeline help |
| M4 | **Website** | FIELD-WEBSITE-COPY.md is draft only | Publish on synapticfour.com CMS |
| M5 | **Lab Kit repo** | External repo may still say `laptop` | PR to Ferrum-Lab-Kit aligning `field-edge` |

### Low / known limits

| # | Area | Limitation |
|---|------|------------|
| L1 | SQLite concurrency | Single-writer; fine for one field node |
| L2 | Beacon index cap | SNV-only, row limit; see FIELD-BEACON-INDEX.md |
| L3 | Crypt4GH Pi throughput | >500 MB/s requires on-device bench; CI ARM runner is smoke only |
| L4 | Regulatory | FIELD-REGULATORY.md is pointers only, not legal advice |
| L5 | Dorado/Guppy | External; Ferrum ingests FASTQ only |

---

## Test coverage after Phase 7

| Layer | Coverage |
|-------|----------|
| Rust workspace | Full `cargo test --workspace` |
| Edge E2E chain | `ci-edge-demo-e2e` → `ci-field-*` (sync, pipeline, ops, ecosystem) |
| HelixTest standard | `conformance.yml` unchanged |
| HelixTest Africa | offline + ont profiles; Ferrum supplements WES/bandwidth/power |
| ARM size | Hard fail ≥ 50 MB on release-edge |

See [TEST-COVERAGE-GAPS.md](TEST-COVERAGE-GAPS.md) for matrix details.

---

## Stakeholder tier statement

**Ferrum Field Edge is at T7 (ecosystem-aligned).** Operators can deploy via `install-field-edge.sh`, run MinION ingest through Beacon query, sync to hub, backup/restore, and operate under solar power constraints — with documented ecosystem wiring to Lab Kit, Demo, and HelixTest.

No further maturity phases are defined in FIELD-MATURITY-PLAN; future work moves to follow-ups F1–F6 and product releases (v0.3 laptop alias removal).
