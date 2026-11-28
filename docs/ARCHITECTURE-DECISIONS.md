# Architecture decisions

This page indexes decision records that already exist. It does not add a decision.

The records are in [DECISIONS.md](../DECISIONS.md). [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) cites ADR-026 and ADR-022 from that file. Other documents cite further numbers. The text of each decision stays in `DECISIONS.md`.

## Numbered records

| Record | Title |
|--------|--------|
| [ADR-012](../DECISIONS.md#2026-06-11---adr-012-embedded-backends-for-resource-constrained-deployments) | Embedded backends for resource-constrained deployments |
| [ADR-013](../DECISIONS.md#2026-06-11---adr-013-outbreak-mode-policy-based-emergency-sharing) | Outbreak Mode (policy-based emergency sharing) |
| [ADR-014](../DECISIONS.md#2026-06-11---adr-014-p2p-federated-beacon-no-central-coordinator) | P2P federated Beacon (no central coordinator) |
| [ADR-015](../DECISIONS.md#2026-06-11---adr-015-append-only-chained-residency-audit-log) | Append-only chained residency audit log |
| [ADR-016](../DECISIONS.md#2026-06-11---adr-016-pluggable-reference-genome-registry) | Pluggable reference genome registry |
| [ADR-017](../DECISIONS.md#2026-06-12---adr-017-external-auth-plane-owned-by-ga4gh-infra-in-co-deploy) | External auth plane owned by ga4gh-infra in co-deploy |
| [ADR-018](../DECISIONS.md#2026-06-18---adr-018-rename-laptop-mode-to-edge-mode) | Rename Laptop Mode to Edge mode |
| [ADR-019](../DECISIONS.md#2026-06-18---adr-019-field-sync-queue-design) | Field sync queue (design) |
| [ADR-020](../DECISIONS.md#2026-06-19---adr-020-offline-jwks-and-edge-operator-accounts) | Offline JWKS and Edge operator accounts |
| [ADR-021](../DECISIONS.md#2026-06-19---adr-021-field-sync-queue-implementation-phase-4) | Field sync queue implementation (Phase 4) |
| [ADR-022](../DECISIONS.md#2026-06-19---adr-022-field-variant-calling-strategy) | Field variant calling strategy |
| [ADR-023](../DECISIONS.md#2026-06-19---adr-023-field-backup-and-integrity-strategy-phase-6) | Field backup and integrity strategy (Phase 6) |
| [ADR-024](../DECISIONS.md#2026-06-19---adr-024-field-ecosystem-alignment-phase-7) | Field ecosystem alignment (Phase 7) |
| [ADR-025](../DECISIONS.md#2026-08-10---adr-025-optional-metadata-store-api-in-ferrum) | Optional Metadata Store API in Ferrum |
| [ADR-026](../DECISIONS.md#2026-09-29---adr-026-one-supported-field-edge-board) | One supported field edge board |
| [ADR-027](../DECISIONS.md#2026-10-03---adr-027-drs-mutations-require-a-bearer-token) | DRS mutations require a bearer token |
| [ADR-028](../DECISIONS.md#2026-10-03---adr-028-discover-broker-jwks-from-the-issuer) | Discover broker JWKS from the issuer |
| [ADR-029](../DECISIONS.md#2026-10-12---adr-029-published-full-infra-gateway-variant) | Published `full-infra` gateway variant |
| [ADR-030](../DECISIONS.md#2026-10-12---adr-030-optional-passport-required-for-drs-byte-access) | Optional Passport required for DRS byte access |
| [ADR-031](../DECISIONS.md#2026-10-12---adr-031-residency-audit-list-requires-authentication) | Residency audit list requires authentication |
| [ADR-032](../DECISIONS.md#2026-10-12---adr-032-authorization_denied-residency-events) | `authorization_denied` residency events |
| [ADR-033](../DECISIONS.md#2026-10-12---adr-033-label-helixtest-westes-stubs-as-simulated) | Label HelixTest WES/TES stubs as simulated |
| [ADR-034](../DECISIONS.md#2026-10-12---adr-034-userinfo-visa-fetch-for-empty-ga4gh_passport_v1-external-auth) | UserInfo Visa fetch for empty `ga4gh_passport_v1` (external auth) |
| [ADR-035](../DECISIONS.md#2026-10-12---adr-035-reference-s3-backend-is-seaweedfs-minio-out-of-compose) | Reference S3 backend is SeaweedFS (MinIO out of Compose) |
| [ADR-036](../DECISIONS.md#2026-11-21---adr-036-an-embedded-empty-visa-list-grants-identity-only-access) | An embedded empty Visa list grants identity-only access (amends ADR-034) |
| [ADR-037](../DECISIONS.md#2026-11-28---adr-037-userinfo-401-or-403-rejects-a-passport-with-an-empty-visa-list) | UserInfo 401 or 403 rejects a Passport with an empty Visa list (amends ADR-036) |
| [ADR-038](../DECISIONS.md#2026-11-28---adr-038-userinfo-visas-are-reused-for-at-most-60-seconds) | UserInfo Visas are reused for at most 60 seconds (amends ADR-034) |

ADR-001 through ADR-011 are not headings in `DECISIONS.md`.

## Records without an ADR number

| Record | Title |
|--------|--------|
| [2026-08-15 image variants](../DECISIONS.md#2026-08-15---named-gateway-image-variants-full--edge--edge-infra) | Named gateway image variants (full / edge / edge-infra) |
| [2026-06-12 ARM64](../DECISIONS.md#2026-06-12---arm64-as-first-class-build-target) | ARM64 as a first-class build target |
| [2026-04-10 baseline](../DECISIONS.md#2026-04-10---establish-cross-repo-quality-and-security-baseline) | Cross-repo quality and security baseline |

The file also contains a blank template heading. That template is not a decision.
