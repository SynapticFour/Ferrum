# Engineering Decisions (ADR-lite)

Track important architectural and operational decisions here.

## Template

### YYYY-MM-DD - Decision title

- **Context:** Why this decision was needed.
- **Decision:** What was chosen.
- **Consequences:** Trade-offs, risks, and follow-up actions.

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
