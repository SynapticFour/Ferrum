# Engineering Decisions (ADR-lite)

Track important architectural and operational decisions here.

## Template

### YYYY-MM-DD - Decision title

- **Context:** Why this decision was needed.
- **Decision:** What was chosen.
- **Consequences:** Trade-offs, risks, and follow-up actions.

---

### 2026-08-15 - Named gateway image variants (full / edge / edge-infra)

- **Context:** Lab Kit selects GA4GH surfaces with runtime `FERRUM_SERVICES__ENABLE_*` on the full monolith image. Field/Pi operators still pull WES/TES/TRS code. Arbitrary per-lab compile matrices in Lab Kit would duplicate Ferrum CI (SBOM, signing, features).
- **Decision:** Ferrum publishes **three** GHCR variants from `deploy/Dockerfile`: **full** (unsuffixed `:<sha>`), **edge** (`--features edge`), **edge-infra** (`edge,external-auth`). Lab Kit maps profiles onto those tags and may wrap `scripts/build-variant-image.sh` for a local/custom architecture. No combinatorial image factory.
- **Consequences:** Beacon+DRS stacks use a smaller binary; htsget remains compiled into `edge` (disable at runtime). Custom cargo feature lists are an escape hatch (`FERRUM_GATEWAY_FEATURES`), not a product surface. GHCR default remains linux/amd64; arm64 is `docker build --platform` / Lab Kit `build image`.

---

### 2026-08-10 - ADR-025: Optional Metadata Store API in Ferrum

- **Status:** Accepted (M0/M1 shipped; M2+ phased)
- **Context:** ferrum-meta is the schema plane; Ferrum already persists submissions in `metadata_submissions` at ingest and binds DRS via `metadata_ref`. Users still could not manage (PUT/GET/list) scientific metadata over HTTP as first-class infra. A full catalog/export service was out of scope for the first shippable claim.
- **Decision:**
  - Keep **ferrum-meta** schema-only.
  - Add optional Ferrum **`[metadata_store]`** (`FERRUM_METADATA_STORE__ENABLED`) mounting **`/api/v1/metadata/*`** over the existing `metadata_submissions` table (document-first).
  - Default **off**; ingest-time storage remains available either way.
  - Auth mirrors ingest: writes need `can_ingest` when `require_auth`; reads need `can_analyze`.
  - **M2:** version history (`metadata_submission_versions`), `If-Match` / `expected_version`, DRS attach/detach.
  - Later phases (JSONB query, Beacon binding, archive export) follow [IMPLEMENTATION-PLAN-METADATA-STORE.md](docs/METADATA-STORE-ROADMAP.md).
- **Consequences:** Hub/Lab-Kit can opt in without Edge paying a default surface. No separate microservice. Not an EGA/GHGA acceptance claim. Clinical SoR stays in Solum.
- **Alternatives considered:** Separate metadata microservice (ops tax); normalize LinkML entities day-1 (schema drift); put scientific description in ADS (wrong plane).

---

### 2026-06-18 - ADR-018: Rename Laptop Mode to Edge mode

- **Status:** Accepted
- **Context:** “Laptop Mode” implied shared lab laptops were the primary target, but those machines typically run the full Docker stack (`make up`). The single-binary embedded deployment targets Raspberry Pi 5, ARM SBCs, and other **edge** nodes at the periphery of a federated genomics network — often offline or intermittently connected.
- **Decision:** Rename user-facing and build terminology to **Edge mode**. Cargo feature `edge` replaces `laptop` (`laptop` remains a deprecated alias for one release cycle). Build profile `release-edge` replaces `release-laptop`. Scripts: `build-edge-native.sh`, `ci-edge-demo-e2e.sh`, Makefile target `make edge`. CLI accepts `--edge` alongside existing `--offline`. Config section `[africa]` unchanged (geographic profile); deployment mode label `edge` in UI/API.
- **Consequences:** Documentation, CI job names, and website copy updated. Prebuilt release artifacts unchanged (same binary contents). Operators using old script names see deprecation warnings. HelixTest and Lab Kit `field-edge` profile naming now align with Ferrum Edge mode.
- **Alternatives considered:** “Embedded mode” (rejected: too implementation-focused); “Field mode” (rejected: overlaps with marketing URL `ferrum-field` but less precise for federation topology).

---

### 2026-06-18 - ADR-019: Field sync queue (design)

- **Status:** Accepted (design); implementation phased per [FIELD-ECOSYSTEM.md](docs/FIELD-ECOSYSTEM.md)
- **Context:** Edge nodes accumulate DRS objects and metadata offline. When connectivity returns, operators need a reliable, resumable, auditable upload path to a hub (cloud S3, national node, or peer Ferrum instance) without re-ingesting from MinION.
- **Decision:** Introduce a **sync queue** as an append-only SQLite table (`sync_queue`) on edge nodes, managed by `ferrum sync` CLI subcommands:
  - `ferrum sync status` — list pending/completed/failed items with byte counts
  - `ferrum sync enqueue [--object-id …] [--all-local]` — mark DRS objects for upstream sync
  - `ferrum sync push --target <url>` — upload pending items (DRS PUT/multipart or hub-specific adapter)
  - `ferrum sync pull` — optional: fetch hub metadata updates (future)
  Each queue entry stores: `object_id`, `target_url`, `state` (pending|in_progress|completed|failed), `bytes_total`, `bytes_sent`, `resume_token`, `crypt4gh` flag, `metadata_bundle_ref` (ferrum-meta submission id), `created_at`, `last_attempt_at`, `error_message`. Upload uses existing `BandwidthMonitor` and `transfer_checkpoints` where applicable. All successful pushes append to `residency_audit`. Crypt4GH objects stream encrypted; plaintext objects may be re-wrapped on push when `[sync] encrypt_on_push = true`.
- **Consequences:** Phase 1 implements schema + CLI skeleton + design doc; full push adapters in Phase 4 of maturity plan. Conflicts (same sample id at hub) resolved by hub policy (reject or version suffix) — documented, not auto-merged. No background daemon required; operator runs `sync push` when link is up (suitable for intermittent VSAT/USB tether).
- **Alternatives considered:** Always-on background sync (rejected: bandwidth and power cost); manual `curl` re-upload (rejected: no resume, no audit); rsync of object directory (rejected: bypasses DRS metadata and consent bindings).

---

### 2026-06-19 - ADR-024: Field ecosystem alignment (Phase 7)

- **Status:** Accepted
- **Context:** Ferrum Edge mode is deployed via Ferrum-Lab-Kit, compared in Ferrum-GA4GH-Demo docs, and validated by HelixTest Africa profiles. Naming drift (`laptop` vs `field-edge`) and Pi-vs-Demo confusion blocked operators.
- **Decision:** Document canonical **`field-edge` / Edge mode** mapping in FIELD-ECOSYSTEM; Pi path uses `install-field-edge.sh` (not Demo Docker on ARM). Supplement HelixTest Africa with Ferrum Rust tests for WES reference mismatch, bandwidth, and power (503). Website copy deck + CLI field i18n (en/fr/de). **Defer** laptop alias removal to v0.3 major; inventory in FIELD-ECOSYSTEM.md.
- **Consequences:** Post-roadmap follow-ups tracked in [FIELD-ECOSYSTEM.md](docs/FIELD-ECOSYSTEM.md); external Lab Kit repo may need separate PR.

---

### 2026-06-19 - ADR-023: Field backup and integrity strategy (Phase 6)

- **Status:** Accepted
- **Context:** Edge Pi nodes need operator-run disaster recovery without Docker/Postgres tooling. Silent bit-rot on USB SSDs can corrupt genomics objects while SQLite metadata still looks valid.
- **Decision:** **`ferrum backup create|restore`** packages SQLite + local `objects/` into a versioned tar.gz. **`ferrum backup verify`** and optional **`[ops] verify_checksums_on_startup`** compare on-disk bytes to DRS SHA-256 checksums and refuse gateway start on mismatch. Solar power behaviour unchanged (503 in emergency); ops docs cover systemd + journald rotation.
- **Consequences:** Operators must stop the gateway before restore. Hub/Postgres deployments use their own backup tools; this ADR applies to Edge SQLite mode only.

---

### 2026-06-19 - ADR-022: Field variant calling strategy

- **Status:** Accepted
- **Context:** Phase 5 closes the MinION → analysis loop. Edge Pi nodes cannot run full GATK/DeepVariant locally; operators need a clear split between field QC and hub variant calling.
- **Decision:** **Default:** lightweight QC on Edge (`ferrum pipeline qc`, NanoStat → `/api/v1/ingest/ont-metrics`); **variant calling** forwarded to hub WES when online (`ferrum pipeline forward-wes`, federated WES in `ferrum-wes`); **local minimap2/small-caller** reserved for future opt-in profile (not Phase 5). VCF results indexed locally via `vcf_index` (capped) for Beacon queries before sync.
- **Consequences:** Hub must expose WES + sufficient compute; offline field nodes queue VCF/FASTQ via Phase 4 sync. See [FIELD-ONT-BASECALLING.md](docs/FIELD-ONT-BASECALLING.md).

---

### 2026-06-19 - ADR-021: Field sync queue implementation (Phase 4)

- **Status:** Accepted
- **Context:** ADR-019 defined the sync queue design; Phase 3 delivered auth for `ferrum:sync_operator`. Operators need resumable hub upload, consent filtering, sneakernet export, and audit trail when connectivity returns.
- **Decision:** Implement `sync_queue` in embedded SQLite; `ferrum sync` CLI (`status`, `enqueue`, `push`, `export`); hub adapter via `/api/v1/ingest/upload` (+ chunked resume); `[sync]` config for DUO/consent policy; optional `/api/v1/sync/*`; `register_on_push` for ga4gh-infra; 409 conflict documented in [docs/FIELD-SYNC-HUB.md](docs/FIELD-SYNC-HUB.md).
- **Consequences:** Edge CLI skips re-running core migrations on existing Edge DBs (`run_migrations=false`). CI: `ci-field-sync-e2e.sh` covers enqueue/push/export and beacon `federate=true` smoke.

---

### 2026-06-19 - ADR-020: Offline JWKS and Edge operator accounts

- **Status:** Accepted
- **Context:** Field Edge nodes may operate without internet for days. JWKS fetch every 5 minutes fails offline; shared MinION laptops need multiple operators without full ga4gh-infra UI.
- **Decision:** Default JWKS cache TTL **7 days**; support **`jwks_file`** for local JSON (no HTTP). Field roles `ferrum:collector`, `ferrum:analyst`, `ferrum:sync_operator` enforced on ingest when `require_auth=true`. SQLite **`edge_operator_accounts`** + `ferrum auth account add|list|login` for PIN-based local tokens. Update bundles may include pre-provisioned JWKS sets. `/health` reports clock skew via NTP probe.
- **Consequences:** Operators rotate keys via USB/signed bundle without broker uptime. See [docs/FIELD-AUTH-OFFLINE.md](docs/FIELD-AUTH-OFFLINE.md).
- **Alternatives considered:** Permanent demo mode on Edge (rejected: no audit trail for multi-user devices).

---

### 2026-06-12 - ADR-017: External auth plane owned by ga4gh-infra in co-deploy

- **Status:** Accepted
- **Context:** Ferrum embedded `ferrum-passports` (broker + visa issuance) duplicated ga4gh-infra's AAI stack. Co-deploying both caused port clashes (8080), split-brain auth, and divergent Passport validation logic.
- **Decision:** When `[auth] mode = "external"` (or `FERRUM_AUTH__MODE=external`), Ferrum disables built-in `/passports/v1`, validates Passports via `ga4gh-clearinghouse`, and optionally registers GA4GH data services in ga4gh-infra's service-registry (`ferrum-discovery`). Standalone Ferrum keeps `mode = "builtin"` unchanged.
- **Consequences:** Co-deploy requires sibling `ga4gh-infra` for path deps (`ga4gh-clearinghouse`, `ga4gh-types`) or published crates. Docker co-deploy uses `deploy/Dockerfile.gateway-monorepo` with monorepo build context. Ferrum owns data plane (DRS, WES, Beacon, …); ga4gh-infra owns identity plane (broker, visas, DUO, ADS, registry).
- **Alternatives considered:** Permanent dual broker in Ferrum (rejected: duplication); Ferrum as sole auth stack (rejected: violates GA4GH AAI separation of concerns).

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
