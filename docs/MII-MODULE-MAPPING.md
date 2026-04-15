# MII Module Mapping (default-17)

This page documents Ferrum MII Connect mapping for the v1 default-17 module set.

## Independent source baseline

Ferrum default-17 scope is aligned to MII module families published by the MII portal:

- Basismodule overview: [Basismodule des Kerndatensatzes der MII](https://www.medizininformatik-initiative.de/de/basismodule-des-kerndatensatzes-der-mii)
- Erweiterungsmodule overview: [Erweiterungsmodule des Kerndatensatzes der MII](https://www.medizininformatik-initiative.de/de/erweiterungsmodule-des-kerndatensatzes-der-mii)
- Version/process tracking: [KDS module version overview](https://github.com/medizininformatik-initiative/kerndatensatz-meta/wiki/%C3%9Cbersicht-%C3%BCber-Versionen-der-Kerndatensatz%E2%80%90Module)

Ferrum stores this as a vendored and versioned manifest under `profiles/mii/manifest.json` for offline/auditable runs. To refresh accepted profile URLs from published FHIR NPM packages, use **`ferrum mii sync-manifest`** with `profiles/mii/sync-spec.json` (see [MII-CONNECT.md](MII-CONNECT.md)).

## Module to resource mapping

Each MII module is shipped as a FHIR NPM package (see `profiles/mii/sync-spec.json`). The manifest lists every `StructureDefinition` the tooling knows about, grouped by FHIR `resourceType` (and a few logical-model identifiers). A single module may therefore cover many resource types.

| MII module | Example FHIR resource types (non-exhaustive) |
|---|---|
| person | `Patient`, `Observation` (e.g. Vitalstatus), … |
| encounter | `Encounter` |
| consent | `Consent`, `DocumentReference`, `Provenance` |
| diagnosis | `Condition` |
| procedure | `Procedure` |
| laboratory | `Observation`, `DiagnosticReport`, `ServiceRequest`, … |
| medication | `MedicationRequest`, `MedicationStatement`, … |
| oncology | `Condition`, `Observation`, `Procedure`, `CarePlan`, … |
| pathology_report | `Composition`, `DiagnosticReport`, `Observation`, … |
| molecular_genetic_report | `DiagnosticReport`, `Procedure`, `Observation`, … |
| molecular_tumor_board | `CarePlan`, `Procedure`, `DiagnosticReport`, … |
| microbiology | `DiagnosticReport`, `Observation` |
| imaging | `ImagingStudy`, `DiagnosticReport`, … |
| intensive_care | `DeviceMetric`, `Observation`, `Procedure`, … |
| biobank | `Specimen`, `Observation`, … |
| document | `DocumentReference` |
| research_study | `ResearchStudy`, `ResearchSubject`, … |

## Profile source

Accepted profile URLs are taken from `profiles/mii/manifest.json`.

Ferrum checks:

1. `resourceType` exists
2. resource maps to an enabled module
3. `meta.profile[]` contains an accepted profile URL for that module/resource

## Gap tags

Current gap tag taxonomy:

- `structure_missing_resource_type`
- `profile_mismatch`
- `missing_meta_profile`
- `missing_profile_rule`
- `outside_manifest_scope`
- `profile_set_version_mismatch`

These tags are intended for CI aggregation and ETL gap triage.
