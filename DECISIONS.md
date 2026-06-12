# Engineering Decisions (ADR-lite)

Track important architectural and operational decisions here.

## Template

### YYYY-MM-DD - Decision title

- **Context:** Why this decision was needed.
- **Decision:** What was chosen.
- **Consequences:** Trade-offs, risks, and follow-up actions.

---

### 2026-06-12 - ARM64 as first-class build target

- **Context:** Raspberry Pi 5 (Cortex-A76, ARM64) is the primary edge hardware for Africa deployments. Hardware AES/SHA2 and NEON acceleration are critical for Crypt4GH performance in field conditions.
- **Decision:** Add target-specific RUSTFLAGS in `.cargo/config.toml` for `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`, and `x86_64-unknown-linux-gnu`. Use the existing `crypt4gh` / RustCrypto stack (ChaCha20-Poly1305 + NEON). Add ARM64 workspace cross-build and Crypt4GH benchmark compile jobs to CI.
- **Consequences:** Crypt4GH encryption on Pi 5 targets **>500 MB/s** throughput when NEON is active. Binary size target **<50 MB** for `ferrum-gateway` on microSD. Cross-compilation in CI uses `gcc-aarch64-linux-gnu` (no physical Pi required).
- **Not decided:** SVE (Scalable Vector Extension) — Pi 5 Cortex-A76 supports SVEv1 but not SVE2. Investigate in a future ADR if throughput targets are not met.

---

### 2026-06-11 - ADR-016: Pluggable reference genome registry

- **Status:** Accepted
- **Context:** African deployments need H3Africa/AWI-GEN and pathogen references as first-class options; hard-coding GRCh38 in WES/Beacon couples Ferrum to a single global reference and hides population-specific accuracy trade-offs.
- **Decision:** `reference_genomes` table with seeded metadata (no auto-download); HTTP registry at `/api/v1/references`; WES `REFERENCE_MISMATCH` warnings for African-origin inputs with global references; Beacon `meta.referenceGenome` for pathogen queries.
- **Consequences:** Operators must ingest and associate FASTA files explicitly. Registry entries are portable across SQLite and PostgreSQL. HelixTest Africa mode validates registry and related paths without changing standard Ferrum conformance.
- **Alternatives considered:** Bundled reference FASTA (rejected: gigabyte repo size); silent auto-selection of African reference (rejected: operator must choose explicitly).

---

### 2026-06-11 - ADR-015: Append-only chained residency audit log

- **Status:** Accepted
- **Context:** African operators need tamper-evident proof of where genomic data went (downloads, federation queries, outbreak events) without relying on external SIEM for baseline compliance.
- **Decision:** `residency_audit` table with per-row `prev_hash` / `entry_hash` over canonical JSON; SQL triggers block UPDATE/DELETE; HTTP query + verify endpoints; no delete API.
- **Consequences:** Chain verification is O(n) over entry count; tampering any row invalidates `chain_valid`. Works on SQLite and PostgreSQL.
- **Alternatives considered:** External blockchain (rejected: operational overhead); mutable application logs (rejected: not tamper-evident).

---

### 2026-06-11 - ADR-014: P2P federated Beacon (no central coordinator)

- **Status:** Accepted
- **Context:** Cross-site pathogen surveillance in Africa often lacks a always-on central Beacon aggregator; sites already trust bilateral peering.
- **Decision:** Config-driven peer list per Ferrum node; opt-in `federate=true` on `GET /g_variants`; parallel fan-out with non-fatal peer failures; union/intersection/local_first aggregation; per-peer rate limits.
- **Consequences:** No HelixTest change when `federate` omitted. Operators must configure peer URLs and optional service tokens. Amplification mitigated by `peer_requests_per_minute`.
- **Alternatives considered:** Central federation hub (rejected: single point of failure); always-on background sync (rejected: bandwidth cost).

---

### 2026-06-11 - ADR-013: Outbreak Mode (policy-based emergency sharing)

- **Status:** Accepted
- **Context:** Countries sharing pathogen genomics quickly during outbreaks may face punitive travel or trade responses. Operators need selective, auditable sharing—not binary share-everything or share-nothing.
- **Decision:** Config-driven **policies** (not ad-hoc manual grants) with explicit activation/deactivation, `outbreak_audit` append-only logging, and two access tiers: **`beacon_only`** (emergency yes/no Beacon bypass for listed recipients) vs **`full`** (Beacon bypass plus separate per-object download approval). GISAID packages generated offline via CLI, not an HTTP endpoint.
- **Consequences:** Requires explicit `[outbreak] enabled = true` and Passport visa `ferrum:outbreak_activator`. Human genomics Beacon paths unchanged when pathogen filters absent. Download approval endpoint prevents silent full-data exfiltration even under emergency policies.
- **Alternatives considered:** Permanent open Beacon for pathogen data (rejected: no audit, no revocation); manual DAC exceptions per query (rejected: not scalable in outbreak timelines).

---

### 2026-06-11 - ADR-012: Embedded backends for resource-constrained deployments

- **Status:** Accepted
- **Context:** African genomics labs often run on shared laptops with intermittent connectivity, without PostgreSQL, MinIO, or container registries. Ferrum previously assumed always-on network and external services at startup.
- **Decision:** Add `ferrum-embed` with SQLite + `LocalStorage` backends, `[africa]` config profile, and `FERRUM_OFFLINE=1`. Production PostgreSQL/S3 paths remain unchanged. Backend selection: `EmbedMode::Full` vs `Sqlite` based on config and env.
- **Consequences:** Single-node laptop deployments need no external services; SQLite has write concurrency limits. DRS, Beacon, and local storage run on SQLite in laptop mode; HelixTest conformance continues on PostgreSQL.
- **Alternatives considered:** Bundled PostgreSQL in Docker (rejected: too heavy for 16 GB laptops); mandatory cloud sync (rejected: violates offline-first requirement).

---

### 2026-04-10 - Establish cross-repo quality and security baseline

- **Context:** Repositories had uneven governance and CI security posture.
- **Decision:** Standardize governance docs, quality gates, and security scanning workflows.
- **Consequences:** Better consistency and contributor trust; ongoing maintenance required to keep checks aligned with stack changes.
