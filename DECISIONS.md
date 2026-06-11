# Engineering Decisions (ADR-lite)

Track important architectural and operational decisions here.

## Template

### YYYY-MM-DD - Decision title

- **Context:** Why this decision was needed.
- **Decision:** What was chosen.
- **Consequences:** Trade-offs, risks, and follow-up actions.

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
