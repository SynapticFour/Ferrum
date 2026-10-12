# Releasing

This repository follows Semantic Versioning (`MAJOR.MINOR.PATCH`).

## Release process

See **[docs/first-release-checklist.md](docs/first-release-checklist.md)** before the next customer tag (`v0.3.0`; `v0.2.0` is already on origin).

1. Run `make verify-release` on the commit you will tag (fmt, clippy, tests, SPDX, cargo-deny when installed). Product CI does not run on push to `main` or on pull requests. Dispatch the workflows in [docs/CI.md](docs/CI.md) when you want HelixTest conformance, Docker TES, ARM, or CodeQL on a runner. A green dispatch is a technical signal, not GA4GH certification.
2. Update `VERSIONS.lock` — pin `FERRUM_VERSION`, `GA4GH_INFRA_REF`, and `HELIXTEST_REF` to compatible **git tags**. Same week: bump Ferrum Lab Kit image pin and Ferrum-GA4GH-Demo `Ferrum-git` / `GA4GH-INFRA-git`. Showcase pins **tags that exist on origin/main** — never SHAs from a rewritten history. Crate `version` in Cargo.toml must equal the git tag (e.g. tag `v0.3.0` → crates `0.3.0`).
3. Update `CHANGELOG.md` with user-visible changes.
4. Create an annotated tag:
   - `git tag -a vX.Y.Z -m "vX.Y.Z"` (e.g. `v0.3.0` — institute-trust / fail-closed)
5. Push the tag:
   - `git push origin vX.Y.Z`
6. Verify GitHub Release assets are present:
   - platform tarballs (`ferrum-gateway-*.tar.gz`)
   - `ferrum-offline-vX.Y.Z.tar.gz` (optional from v0.3.6 — Compose images + `import.sh` + Helm; may be omitted if third-party pulls fail)
   - `ferrum-helm-vX.Y.Z.tgz` (optional Kubernetes path)
   - `SHA256SUMS.txt` (covers binaries, offline bundle when present, Helm, SBOM, `VERSIONS.lock`, `install.sh`)
   - `ferrum-sbom.cdx.json`
   - `VERSIONS.lock`
7. Verify generated release notes and smoke-test installation from release artifacts.

## Manual GitHub Release for existing tag `v0.3.5`

Tag `v0.3.5` is on origin; the Release workflow failed on `docker pull minio/minio` (401). Binary artifacts from run [38251721231](https://github.com/SynapticFour/Ferrum/actions/runs/38251721231) remain. **Do not move the tag.** Create the Release by hand (no offline tarball unless you build one locally with SeaweedFS):

```bash
# From a clean clone at tag v0.3.5 (or ae3762b0):
git fetch origin tag v0.3.5
git checkout v0.3.5

mkdir -p /tmp/ferrum-v035-release && cd /tmp/ferrum-v035-release
gh run download 38251721231 -R SynapticFour/Ferrum \
  -n ferrum-gateway-x86_64-unknown-linux-musl \
  -n ferrum-gateway-aarch64-unknown-linux-musl \
  -n ferrum-gateway-x86_64-apple-darwin \
  -n ferrum-gateway-aarch64-apple-darwin

# Artifacts unpack as directories or tarballs named ferrum-gateway-*.tar.gz — collect:
find . -name 'ferrum-gateway-*.tar.gz' -print

# Optional: Helm chart from that commit
git -C /path/to/Ferrum archive --format=tar.gz --prefix=ferrum-helm/ v0.3.5:deploy/helm \
  >/dev/null 2>&1 || true
# Prefer: helm package at that commit
#   helm package deploy/helm --version 0.3.5 --appVersion v0.3.5 -d .
#   mv ferrum-0.3.5.tgz ferrum-helm-v0.3.5.tgz

# Optional SBOM (needs rust toolchain at that commit):
#   cargo install cargo-cyclonedx --locked --version 0.5.9
#   cargo cyclonedx --manifest-path crates/ferrum-gateway/Cargo.toml --format json --override-filename ferrum-sbom.cdx
#   cp … ferrum-sbom.cdx.json

cp /path/to/Ferrum/VERSIONS.lock /path/to/Ferrum/install.sh /path/to/Ferrum/import.sh .

shasum -a 256 ferrum-gateway-*.tar.gz ferrum-helm-v0.3.5.tgz \
  ferrum-sbom.cdx.json VERSIONS.lock install.sh import.sh > SHA256SUMS.txt 2>/dev/null \
  || shasum -a 256 ferrum-gateway-*.tar.gz VERSIONS.lock install.sh import.sh > SHA256SUMS.txt

gh release create v0.3.5 \
  --repo SynapticFour/Ferrum \
  --verify-tag \
  --title "v0.3.5" \
  --notes "Gateway binaries for tag v0.3.5. Offline image bundle omitted (MinIO Hub 401 at release time). Use SeaweedFS reference Compose from a later commit / proposed v0.3.6. GHCR images: ferrum:v0.3.5-full-infra, ferrum-ui:v0.3.5." \
  ferrum-gateway-*.tar.gz \
  SHA256SUMS.txt \
  VERSIONS.lock \
  install.sh \
  import.sh \
  ferrum-helm-v0.3.5.tgz \
  ferrum-sbom.cdx.json
# Omit last two lines if those files were not produced.
```

Offline bundle for `v0.3.5` is intentionally skipped in this manual path; rebuild with post-U2 `export_offline_bundle.sh` (SeaweedFS) only if you need it and are willing to attach a later-built artifact to the same Release.

## Versioning rules

- `MAJOR`: breaking API/behavior changes
- `MINOR`: backward-compatible features
- `PATCH`: backward-compatible fixes and maintenance

## Backport policy

Security fixes should be backported to actively maintained release lines where feasible.
