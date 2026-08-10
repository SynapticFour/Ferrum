# Implementation Plan — Optional Metadata Store (Ferrum)

**Status:** M0–M2 done (2026-08-10); M3+ open
**Date:** 2026-08-10
**Owner:** Ferrum product / Synaptic Four platform
**Related:** [ferrum-meta](https://github.com/SynapticFour/ferrum-meta), `ferrum-meta-connect`, Field T3 (`metadata_ref` / `metadata_submissions`), ADR-025, [METADATA-STORE.md](../METADATA-STORE.md)

---

## 0. Honest starting point

Synaptic Four already **stores** scientific metadata inside Ferrum:

| Piece | Where | What it does today |
|-------|--------|-------------------|
| Schema / profiles | **ferrum-meta** (LinkML) | Study → Individual → Sample → Experiment → File → Dataset |
| Offline validate | `ferrum-meta-connect` | Structural subset (`core` / `pathogen` / `h3africa`) |
| Persist | `metadata_submissions` | Opaque JSON document keyed by alias |
| Bind | `drs_objects.metadata_ref` | Alias pointer on DRS objects |
| Ingest write | `POST /api/v1/ingest/register` (+ ONT / watch) | Validate → upsert → link |
| Sync | Field queue / export | Carries `metadata_ref` / bundle on hub push |

What users **cannot** do yet (the real gap):

1. **CRUD / read** submissions via a public API (only write-at-ingest + internal sync reads)
2. **List / search** by study, sample, DUO, profile, individual
3. **Version** submissions (today: last-write-wins upsert)
4. **Resolve** `metadata_ref` from a DRS GET into the full document over HTTP
5. **Export** to archive formats (EGA / GHGA / ENA) — crosswalks exist as docs only
6. **Drive Beacon / ADS** from ferrum-meta entities (parallel indexes today)

So the ask is not “invent metadata storage.” It is: **promote the field ingest side-table into an optional, first-class Metadata Store product surface** inside Ferrum.

---

## 1. Is this the last gap for “all genomic / medical data”?

**No — it is the last major gap in the *genomic submission / archive metadata* plane**, and a high-leverage one. Other planes remain intentional companions:

| Data class | Where it lives today | Gap? |
|------------|----------------------|------|
| Object bytes + checksums | Ferrum DRS / storage | Covered |
| Workflow runs / tasks | WES / TES | Covered |
| Tool registry | TRS | Covered |
| Scientific submission metadata | ferrum-meta → opaque DB | **Store yes; manage/query/export no** |
| Discovery index | Beacon v2 tables | Parallel; not filled from ferrum-meta |
| Access / visas / DUO match | Passports + ga4gh-infra | Identity plane — not scientific catalog |
| Clinical FHIR / openEHR / consent SoR | **Solum** (+ MII Connect gate) | Separate regulatory perimeter by design |
| Phenotype packets (Phenopackets) | BRA / ad-hoc; not a Ferrum SoR | Soft gap if clinics need phenotype SoR in-stack |
| Imaging (DICOM) | Out of scope | Not Ferrum |
| Biospecimen LIMS / freezer inventory | Out of scope | Partner / Lab-Kit ops, not platform SoR |
| Variant annotation / secondary findings DB | External / WES outputs as DRS files | Usually file-backed, not a new meta store |
| Literature / RAG knowledge | BioResearch Assistant | Separate product |
| Run provenance / RO-Crate | HELIOS + Ferrum provenance | Covered for pipelines |
| EHDS “catalog” RDF/JSON-LD | Website / project language | Not shipped — often fed *from* metadata store |

**Verdict:** Shipping a Metadata Store closes the “we can hold the files but not curate/query the scientific description” story for genomics. It does **not** replace Solum for clinical SoR, nor imaging/LIMS. Optional later bridges (Phenopackets projection, Beacon backfill, Solum subject-link) are separate milestones.

---

## 2. Design principles (proposal)

1. **Keep ferrum-meta as schema plane** — no DB in ferrum-meta; no runtime service there.
2. **Ferrum owns runtime** — optional module/feature, default-off or default-on-for-hub / off-for-minimal-edge via config.
3. **Document-first, project later** — keep the submission JSON as source of truth; add projections for query without requiring a full LinkML ORM on day 1.
4. **Reuse `metadata_ref`** — DRS stays the object plane; metadata store is the document plane; binding stays alias-based.
5. **Auth same as DRS/WES** — `require_auth`, Passports, Solum teeth where configured; no anonymous write.
6. **Offline-first preserved** — Edge still validates + stores locally; hub API is the online management surface.
7. **Honesty** — “Metadata Store” ≠ archive acceptance; export to EGA/GHGA is a later optional stage.

---

## 3. Recommended approach (progressive)

### Why not the alternatives

| Option | Verdict |
|--------|---------|
| **A. Document API over existing table + versions + list** | **Start here** — smallest honest product, builds on T3 |
| **B. Fully normalized Study/Sample/… tables only** | Too heavy day 1; schema drift vs LinkML; do as **projections** after A |
| **C. Separate microservice** | Extra ops tax; fights “optional Ferrum module” goal |
| **D. Stuff into ADS / Passports** | Wrong plane (access ≠ scientific description) |
| **E. Only improve CLI / files on disk** | Insufficient for “infra stores our metadata” customer claim |

**Path:** A → selective B (JSONB / GIN or thin projection tables) → export / Beacon binding.

### Suggested product name / surface

- Config: `[metadata_store] enabled = true` (or Cargo feature `metadata-store` + config)
- HTTP prefix (proposal): **`/api/v1/metadata/...`** (Ferrum-native)
  Optional later alias: `/ga4gh/...` only if a GA4GH product fits; do **not** invent a fake GA4GH path.
- CLI: extend `ferrum meta` with `push` / `get` / `list` against local or remote store

---

## 4. Phased implementation

### M0 — Framing (docs / ADR) — **done 2026-08-10**

- ADR-025 in Ferrum `DECISIONS.md`: Metadata Store is optional Ferrum runtime; ferrum-meta remains schema-only.
- [METADATA-STORE.md](../METADATA-STORE.md) + `profiles/meta/README.md` honesty.
- Customer one-pager: what it proves / does not prove (no EGA acceptance claim).

**Exit:** Documented contract. ✅

### M1 — Read/Write API over existing store — **done 2026-08-10**

Expose what already exists:

| Method | Path | Behaviour |
|--------|------|-----------|
| `PUT` / `POST` | `/api/v1/metadata/submissions` | Validate (`ferrum-meta-connect`) → upsert by alias |
| `GET` | `/api/v1/metadata/submissions/{alias}` | Return full document + profile |
| `GET` | `/api/v1/metadata/submissions` | List (alias, profile, created_time); pagination |

Also:

- Auth gates consistent with ingest / analyze roles
- Soft feature flag: if disabled, routes **501**
- Tests: `crates/ferrum-drs/tests/metadata_store_api.rs`

**Exit:** curl/CLI can store and retrieve a submission without going through file ingest. ✅

### M2 — Versioning & attach/detach — **done 2026-08-10**

- `metadata_submissions` columns: `version`, `updated_time`, `content_sha256`
- History table `metadata_submission_versions`
- `GET /api/v1/metadata/submissions/{alias}/versions` (+ `.../versions/{n}`)
- `PUT /api/v1/metadata/objects/{object_id}/metadata_ref` — attach/detach
- Optimistic concurrency: `If-Match: "<version>"` or `?expected_version=` → **409** on mismatch; response `ETag`

**Exit:** Auditable history; DRS link management without re-ingest. ✅

### M3 — Query projections (optional depth) — ~1–2 weeks

Minimal viable query without full normalization:

- Postgres: `document JSONB` + GIN; indexes on `alias`, `profile`, `$.datasets[*].id`, DUO codes
- SQLite Edge: keep document; limited `json_extract` filters or skip heavy query on Edge
- Endpoints: filter by `profile`, `study_alias`, `duo`, `individual_id`

Optional thin projection tables (`meta_studies`, `meta_samples`) filled on write — only if GIN proves insufficient.

**Exit:** “Find all samples with DUO:0000007 in profile h3africa” works on Hub.

### M4 — Ecosystem binding — ~1 week

- Sync queue already carries refs — ensure hub Metadata Store is source of truth after push
- Optional: Beacon biosample/individual **backfill job** from submission (feature-flagged; never overwrite curated Beacon rows silently)
- Document Passport/ADS `dataset_id` ↔ ferrum-meta Dataset alias convention (mapping table or naming rule)

**Exit:** One documented story from meta Dataset → DRS objects → (optional) Beacon/ADS.

### M5 — Archive export (optional) — ~2–4 weeks (can trail)

- Transpilers: ferrum-core → GHGA TSV / EGA Webin-shaped packages (start with one archive)
- Crosswalks already live in ferrum-meta docs — implement as Ferrum crate or Python sidecar invoked by CLI
- Soft-fail if profile incomplete

**Exit:** `ferrum meta export --profile ghga` produces reviewable artefacts (not “accepted by archive”).

### M6 — Productization — continuous

- Ferrum-GA4GH-Demo / Showcase stage: store → get → Evidence Pack hash of submission
- HelixTest: only if we claim a stable public contract worth conformance
- Lab-Kit: enable `[metadata_store]` on hub profile; keep Edge write-via-ingest

---

## 5. Suggested module layout

```
Ferrum/
  crates/ferrum-meta-connect/     # unchanged: validate
  crates/ferrum-drs/              # keep ingest binding; call into store
  crates/ferrum-metadata/         # NEW (optional): API, versioning, query
  migrations/                     # version columns, JSONB if needed
```

Gateway mounts routes when `metadata_store.enabled`.
Edge binary: same codepaths; query endpoints may be hub-only via config `mode = "full" | "ingest_only"`.

---

## 6. Non-goals (keep honest)

- Not a clinical CDR / FHIR store (Solum)
- Not DICOM / PACS
- Not a replacement for institutional LIMS
- Not full LinkML-in-Rust parity (Python remains schema SoT; Rust stays structural + profile subset until justified)
- Not automatic EGA/GHGA acceptance
- Not merging Beacon entity model into LinkML entities on day 1

---

## 7. Rough priority recommendation

| Priority | Milestone | Why |
|----------|-----------|-----|
| P0 | M0 + M1 | ✅ Turns “we store blobs” into “users can manage metadata in our infra” |
| P1 | M2 | ✅ Required for real curation / audit conversations |
| P2 | M3 | Required for “search our catalog” demos |
| P3 | M4 | Portfolio coherence (Beacon/ADS) |
| P4 | M5–M6 | Archive + Showcase polish |

**Smallest shippable claim after M1:**
“Synaptic Four Hub can validate, store, list, and retrieve ferrum-meta submissions over HTTP, linked to DRS objects via `metadata_ref`.”

---

## 8. Open decisions (need product call)

1. **Default on or off?** Proposal: **off** on minimal Edge; **on** for Hub / Lab-Kit `hub` profile.
2. **Expand DRS vs sibling API?** Proposal: sibling `/api/v1/metadata` + optional `expand=metadata` on DRS GET.
3. **Phenopackets:** store-as-attachment inside submission vs separate resource — defer to post-M2.
4. **Who owns GHGA/EGA export code?** Ferrum crate vs small Python tool next to ferrum-meta — prefer Ferrum CLI calling shared logic so ops stay one binary where possible.

---

## 9. Success metrics

- M1: API round-trip in CI; Demo smoke optional
- M2: two versions of same alias; DRS still points at current
- M3: at least three filter queries documented + tested
- Customer narrative: Showcase/Demo can show file **and** scientific description under Synaptic Four control without external catalog DB
