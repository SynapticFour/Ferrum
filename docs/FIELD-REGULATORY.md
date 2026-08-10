# Field regulatory pointers (Phase 6)

Ferrum is infrastructure — **not** a certified medical device or LIMS. Operators remain responsible for national compliance. This guide lists common considerations for African field genomics deployments.

Related: [COMPLIANCE.md](COMPLIANCE.md), [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md), [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md).

## GDPR (EU citizens / EU-funded studies)

When EU personal data is processed on an Edge node:

- **Lawful basis & DUO:** Encode consent in ferrum-meta submissions; enforce via `[sync] allowed_duo_codes` before hub push.
- **Data minimisation:** Sync only objects required for the study; use sneakernet export with policy filters.
- **Right of access / erasure:** DRS object deletion + residency audit trail; hub must honour erasure requests for synced copies.
- **Cross-border transfer:** Document hub location and Standard Contractual Clauses where applicable.
- **DPIA:** Required when processing genetic data at scale; Ferrum audit logs support accountability (Art. 30 records).

## African national health & research law (non-exhaustive)

Consult local counsel. Common themes:

| Region / framework | Consideration |
|--------------------|---------------|
| **AU Data Policy Framework / national DPAs** | Local storage default (Edge mode); residency audit for cross-border Beacon queries |
| **H3Africa / national ethics boards** | IRB approval numbers in ferrum-meta; outbreak mode requires explicit activator role |
| **South Africa POPIA** | Lawful processing, security safeguards; breach notification to Information Regulator |
| **Kenya Data Protection Act** | Registration with ODPC for controllers; cross-border transfer rules. When co-deploying **Solum** `kenya-dpa`, follow Solum [H4-OFFLINE-SYNC-POLICY.md](https://github.com/SynapticFour/Solum/blob/main/docs/H4-OFFLINE-SYNC-POLICY.md) — profile is **PROVISIONAL** until counsel; empty transfer destinations stay fail-closed. Showcase: [H4-PILOT-CHECKLIST.md](https://github.com/SynapticFour/SynapticFour-Showcase/blob/main/docs/pilots/H4-PILOT-CHECKLIST.md). |
| **Nigeria NDPA 2023** | Consent and purpose limitation for identifiable genomic data |
| **WHO outbreak sharing** | GISAID metadata + outbreak policies; emergency Beacon access is logged |

## Operational controls Ferrum provides

- Append-only **residency audit** (`GET /api/v1/audit/residency`)
- **Crypt4GH** encryption at rest (optional on ingest)
- **Edge operator accounts** with role-scoped visas (collector / analyst / sync)
- **Offline-first** operation without mandatory cloud dependency
- **Backup/restore** CLI for disaster recovery ([FIELD-OPS.md](FIELD-OPS.md))

## What Ferrum does not provide

- Legal review of study protocols
- Automatic anonymisation / re-identification risk scoring
- Country-specific e-consent UI
- Certified ISO 13485 / IVDR documentation

For enterprise compliance packs, see [COMPLIANCE.md](COMPLIANCE.md) and engage your institutional DPO.
