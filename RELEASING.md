# Releasing

This repository follows Semantic Versioning (`MAJOR.MINOR.PATCH`).

## Release process

1. Ensure all required CI workflows are green on `main` (build, lint, tests, conformance).
2. Update `CHANGELOG.md` with user-visible changes.
3. Create an annotated tag:
   - `git tag -a vX.Y.Z -m "vX.Y.Z"` (e.g. `v0.2.0` — Africa resilience release)
4. Push the tag:
   - `git push origin vX.Y.Z`
5. Verify GitHub Release assets are present:
   - platform tarballs (`ferrum-gateway-*.tar.gz`)
   - `SHA256SUMS.txt`
   - `ferrum-sbom.cdx.json`
6. Verify generated release notes and smoke-test installation from release artifacts.

## Versioning rules

- `MAJOR`: breaking API/behavior changes
- `MINOR`: backward-compatible features
- `PATCH`: backward-compatible fixes and maintenance

## Backport policy

Security fixes should be backported to actively maintained release lines where feasible.
