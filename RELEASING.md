# Releasing

This repository follows Semantic Versioning (`MAJOR.MINOR.PATCH`).

## Release process

See **[docs/first-release-checklist.md](docs/first-release-checklist.md)** before the next customer tag (`v0.3.0`; `v0.2.0` is already on origin).

1. Ensure all required CI workflows are green on `main` (build, lint, tests, conformance).
2. Update `VERSIONS.lock` — pin `FERRUM_VERSION`, `GA4GH_INFRA_REF`, and `HELIXTEST_REF` to compatible **git tags**. Same week: bump Ferrum Lab Kit image pin and Ferrum-GA4GH-Demo `Ferrum-git` / `GA4GH-INFRA-git`. Showcase pins **tags that exist on origin/main** — never SHAs from a rewritten history. Crate `version` in Cargo.toml must equal the git tag (e.g. tag `v0.3.0` → crates `0.3.0`).
3. Update `CHANGELOG.md` with user-visible changes.
4. Create an annotated tag:
   - `git tag -a vX.Y.Z -m "vX.Y.Z"` (e.g. `v0.3.0` — institute-trust / fail-closed)
5. Push the tag:
   - `git push origin vX.Y.Z`
6. Verify GitHub Release assets are present:
   - platform tarballs (`ferrum-gateway-*.tar.gz`)
   - `ferrum-offline-vX.Y.Z.tar.gz` (Compose images + `import.sh` + Helm chart)
   - `ferrum-helm-vX.Y.Z.tgz` (optional Kubernetes path)
   - `SHA256SUMS.txt` (covers binaries, offline bundle, Helm, SBOM, `VERSIONS.lock`, `install.sh`)
   - `ferrum-sbom.cdx.json`
   - `VERSIONS.lock`
7. Verify generated release notes and smoke-test installation from release artifacts.

## Versioning rules

- `MAJOR`: breaking API/behavior changes
- `MINOR`: backward-compatible features
- `PATCH`: backward-compatible fixes and maintenance

## Backport policy

Security fixes should be backported to actively maintained release lines where feasible.
