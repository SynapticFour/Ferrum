# MII CI Integration

Ferrum MII Connect is designed to be CI-friendly for DIZ ETL pipelines.

## GitHub Actions

Minimal step:

```yaml
- name: MII KDS validation
  run: |
    cargo run -p ferrum-cli -- mii validate \
      --input ./etl-artifacts/fhir \
      --manifest profiles/mii/manifest.json \
      --strict \
      --format sarif \
      --output mii-report.sarif

- name: Upload report
  uses: actions/upload-artifact@v4
  with:
    name: mii-report
    path: mii-report.sarif
```

Ferrum repository ships:

- optional CI step in `.github/workflows/ci.yml` (controlled by `RUN_MII_CONNECT`)
- dedicated `.github/workflows/mii-conformance.yml`

## Regenerating the manifest (optional)

In controlled environments you can refresh `profiles/mii/manifest.json` from pinned packages before CI:

```bash
cargo run -p ferrum-cli -- mii sync-manifest \
  --spec profiles/mii/sync-spec.json \
  --output profiles/mii/manifest.json \
  --cache-dir profiles/mii/package-cache
```

Commit the updated manifest (and optionally the `package-cache` mirrors only if your policy allows vendoring binaries; otherwise rely on `--offline` with a pre-seeded cache in secure build agents). See [MII-CONNECT.md](MII-CONNECT.md) for limitations.

## GitLab CI

```yaml
mii_validate:
  stage: test
  script:
    - cargo run -p ferrum-cli -- mii validate --input ./fhir --manifest profiles/mii/manifest.json --strict --format json --output mii-report.json
  artifacts:
    when: always
    paths:
      - mii-report.json
```

## Suggested pipeline policy

- strict mode for production/protected branches
- non-strict mode for early ETL iterations
- store report artifacts with pipeline id + commit sha

## Audit retention recommendation

Keep for each validation run:

- report (`json`/`sarif`)
- manifest (`profiles/mii/manifest.json`) used in the run
- Ferrum version + commit SHA
- pipeline id + timestamp
