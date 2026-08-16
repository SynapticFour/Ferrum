# ferrum-meta profiles (Ferrum Edge)

Offline validation and fixtures for [ferrum-meta](https://github.com/SynapticFour/ferrum-meta) field collection metadata.

## Profiles

| Profile | Study `type` | Use case |
|---------|--------------|----------|
| `core` | Any ferrum-core type | Minimal structural validation |
| `pathogen` | `PATHOGEN_SURVEILLANCE`, `OUTBREAK_RESPONSE` | AMR / outbreak field surveillance |
| `h3africa` | `H3AFRICA` | Population genomics with consent + country |

## CLI

```bash
# Validate (profile auto-detected from study type)
ferrum meta validate --input profiles/meta/fixtures/ferrum-pathogen-minimal-submission.yaml

# Generate template (interactive)
ferrum meta init --profile pathogen --output /tmp/collection.yaml

# Non-interactive
ferrum meta init --profile h3africa --output /tmp/h3a.yaml \
  --study-title "Pilot" --sample-alias s001 --country Kenya --non-interactive

# Paper form → YAML (CSV header documented in docs/AFRICA-DEPLOYMENT.md)
ferrum meta import --profile pathogen --csv forms/row.csv --output /tmp/collection.yaml
```

## Ingest binding

At ingest, attach metadata via:

- `POST /api/v1/ingest/register` — JSON fields `ferrum_meta`, `metadata_ref`, optional `metadata_profile`
- `POST /api/v1/ingest/ont` — multipart field `ferrum_meta` (YAML or JSON)
- `ferrum ingest watch --meta-bundle collection.yaml --collector "Dr. A"`

Validated bundles are stored in `metadata_submissions`; DRS objects get `metadata_ref` (dataset alias).

## Optional Metadata Store API (M1/M2)

To manage submissions over HTTP (without re-ingest), enable:

```toml
[metadata_store]
enabled = true
```

Then:

- `PUT /api/v1/metadata/submissions/{alias}` — validate + upsert (optional `If-Match: "<version>"`)
- `POST /api/v1/metadata/submissions` — validate + upsert
- `GET /api/v1/metadata/submissions/{alias}` — current document + version
- `GET /api/v1/metadata/submissions` — list
- `GET /api/v1/metadata/submissions/{alias}/versions` — history
- `PUT /api/v1/metadata/objects/{id}/metadata_ref` — attach/detach

See [docs/METADATA-STORE.md](../../docs/METADATA-STORE.md) and ADR-025.

## Fixtures

| File | Profile |
|------|---------|
| `ferrum-core-minimal-submission.yaml` | core |
| `ferrum-pathogen-minimal-submission.yaml` | pathogen |
| `ferrum-h3africa-minimal-submission.yaml` | h3africa |

## Schema sync

Vendored LinkML YAML lives in [`schema/`](schema/) and is a **compile-time input** of `ferrum-meta-connect` (`include_str!`). The Rust validator is a **structural subset** of that YAML, not a LinkML runtime. Full profile parity (e.g. `PathogenSample`, `consent_records`) stays in the ferrum-meta Python toolchain.

```bash
# Refresh from a sibling ferrum-meta checkout (or FERRUM_META_REPO)
./scripts/sync-ferrum-meta-schemas.sh

# CI / local: fail if vendored YAML drifted from the pin in sync-spec.json
./scripts/sync-ferrum-meta-schemas.sh --check
```

GHGA / EGA starter bundles (full LinkML check stays in ferrum-meta):

```bash
ferrum meta export --profile ghga --output ./ghga-bundle.yaml
ferrum meta export --profile ega --output ./ega-bundle.yaml
../ferrum-meta/scripts/validate-fixture.sh ./ghga-bundle.yaml
```

Pin: `VERSIONS.lock` `FERRUM_META_SHA` and `profiles/meta/sync-spec.json`.
