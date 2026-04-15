# Ferrum MII Connect

Ferrum MII Connect adds a **technical** conformance layer: it checks incoming FHIR instances against **vendored profile metadata** (canonical StructureDefinition URLs and resource types) derived from MII-oriented FHIR NPM packages. It is **not** a full HL7 FHIR validator, not a replacement for institutional DIZ certification processes, and **not** legal advice about regulatory compliance.

## Scope (v1)

Default module set (default-17):

- person
- encounter (Fall/Behandlungsfall)
- consent
- diagnosis
- procedure
- laboratory
- medication
- oncology
- pathology_report
- molecular_genetic_report
- molecular_tumor_board
- microbiology
- imaging
- intensive_care
- biobank
- document
- research_study

Compatibility aliases are still accepted for legacy `genomics`, plus `rare_diseases` and `pros_proms` (optional, not in the default-17 set).

## Offline-first profile model

Ferrum ships vendored profile metadata under `profiles/mii/`:

- `profiles/mii/manifest.json`
- `profiles/mii/samples/default17-bundle.json`

Validation does not require runtime internet access by default.

## Deterministic manifest sync (`mii sync-manifest`)

To regenerate `profiles/mii/manifest.json` from **pinned** package versions on the public FHIR package registry (`packages.fhir.org` by default), use:

```bash
cargo run -p ferrum-cli -- mii sync-manifest \
  --spec profiles/mii/sync-spec.json \
  --output profiles/mii/manifest.json \
  --cache-dir profiles/mii/package-cache
```

- **Online (default):** downloads each `.tgz` once, writes a **content-addressed mirror** under `--cache-dir`, then extracts all `StructureDefinition` resources to build `accepted_profiles` and per-rule checksums. Each package row includes `package_sha256` for audit.
- **Air-gapped / reproducible builds:** populate `package-cache` out-of-band, then run with `--offline` so no network is required.

**Version pins:** `profiles/mii/sync-spec.json` lists `package_name` and `package_version` per module (NPM ids such as `de.medizininformatikinitiative.kerndatensatz.laborbefund` for laboratory, `de.medizininformatikinitiative.kerndatensatz.patho` for pathology, `de.medizininformatikinitiative.kerndatensatz.icu` for intensive care, `de.medizininformatikinitiative.kerndatensatz.studie` for research study). The committed manifest was generated from versions that resolve on `packages.fhir.org`; change pins when your governance or MII releases require it.

**What sync does not do:** it does not execute Simplifier.net or HL7 Java validator rules, does not validate terminology bindings, and does not prove end-to-end interoperability with a specific DIZ test suite. It produces a **deterministic, auditable manifest** for Ferrum’s lightweight `meta.profile` checks.

## CLI usage

Validate one file:

```bash
cargo run -p ferrum-cli -- mii validate \
  --input profiles/mii/samples/default17-bundle.json \
  --manifest profiles/mii/manifest.json \
  --strict \
  --output mii-report.json \
  --format json
```

Validate a directory (all files merged as one batch):

```bash
cargo run -p ferrum-cli -- mii validate \
  --input ./etl-output/fhir \
  --manifest profiles/mii/manifest.json \
  --format sarif \
  --output mii-report.sarif
```

## Exit codes

- `0`: no blocking findings
- `1`: conformance violations (`error`) or strict gap failure
- `2`: runtime/config/read errors (wrapper/pipeline level)

## Report fields

Per resource:

- `resource_id`
- `resource_type`
- `module`
- `profile`
- `status`
- `issues[]`
- `gap_tags[]`

Envelope:

- `generated_at`
- `profile_set_version`
- `manifest_sha256`
- `summary`
- `gap_list`

## Configuration

Use config or env overrides:

- `[mii_connect]` in `config.toml`
- `FERRUM_MII_CONNECT__*` environment variables

See [MII-CI-INTEGRATION.md](MII-CI-INTEGRATION.md) for CI usage patterns.

## Reference sources (public MII materials)

Module families and release process are documented by the MII (e.g. Basismodule / Erweiterungsmodule pages and the `kerndatensatz-meta` version wiki). Ferrum uses that material for **scope alignment** only; product documentation does not claim endorsement by the MII or any regulator.

Operational implications:

- Keep `profiles/mii/manifest.json` and (when used) `sync-spec.json` versioned in-repo or in signed release artifacts.
- Bump `profile_set_version` whenever package versions or accepted profile URLs are updated.
- Retain validation reports (`json` or `sarif`) with manifest checksum for audit trails.
