# CONTINUITY — Ferrum

Survivability notes for operators and successors. Describes **what is true on `origin/main` today**, not a roadmap. Not legal advice.

Checked on 2026-10-14. Release tags: `git tag` (do not copy a tag into this file). Companion pins and crate versions: `VERSIONS.lock`.

## What this project is

On-premises GA4GH data/compute plane in Rust: DRS, WES, TES, TRS, Beacon v2, htsget, Passports, Crypt4GH — one gateway. License: **BUSL-1.1** (source-available; not OSI open source today). Maturity: Active (beta). HelixTest CI is a technical signal, not GA4GH certification.

Related public repos (separate versioning): [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra), [HelixTest](https://github.com/SynapticFour/HelixTest), [Helix](https://github.com/SynapticFour/Helix), [Ferrum-Lab-Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit), [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo).

## Build from a clean machine (from a given tag)

### Prerequisites (must be installed first)

- Git
- [rustup](https://rustup.rs) — toolchain **1.91.1** (see `rust-toolchain.toml`; components `rustfmt`, `clippy`)
- `make`
- For native builds on Debian/Ubuntu: `pkg-config`, `libssl-dev`, and typically `libpq-dev`, `protobuf-compiler`, `python3` (`make prove` runs `scripts/spdx-rs.py` and OpenAPI checks)
- For demo / TES Docker paths: Docker Engine + Compose plugin
- Network access to **crates.io** (and Docker Hub / GHCR when pulling images)
- Optional: `cargo-deny` for `make verify-release` licence/advisory gates
- Low-memory hosts: `CARGO_BUILD_JOBS=1` avoids parallel link OOM during large workspace builds

### Source checkout

```bash
git clone https://github.com/SynapticFour/Ferrum.git
cd Ferrum
git tag -l 'v*'       # pick a tag that exists; pins for a release are in VERSIONS.lock
git checkout <tag>
# For current main instead of a tag:
# git checkout main && git pull
```

Pinned companions for a release are in `VERSIONS.lock` (`FERRUM_VERSION`, `GA4GH_INFRA_REF`, `HELIXTEST_REF`).

### Compile / unit proof (no Docker)

```bash
make prove    # cargo test --workspace --all-targets
```

Equivalent gates used before release (see `RELEASING.md`):

```bash
make verify-release   # fmt, clippy, tests, SPDX, cargo-deny when installed
```

### Demo stack (Docker)

```bash
make up       # demo — TES noop; auth off
# or:
make up-tes   # demo + Docker-backed TES
make down     # stop, keep volumes
make destroy  # remove volumes and project images
```

Installer / evaluator paths: `docs/GETTING-STARTED.md`, `docs/FOR-EVALUATORS.md`, `docs/INSTALLATION.md`.

## How to run tests

| Command | Meaning |
|---------|---------|
| `make prove` / `cargo test --workspace --all-targets` | Default offline/workspace tests |
| `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` | Lint gates (`TESTING.md`) |
| `make test-tes` after `make up-tes` | Ingest / Crypt4GH / WES COMPLETE smoke |
| `make smoke-pilot` | Broader pilot smoke (see `TESTING.md`) |
| Dispatch workflows in `docs/CI.md` | Product CI is **not** automatic on every push/PR for all suites |

## How to cut a release

Follow `RELEASING.md` and `docs/first-release-checklist.md`:

1. `make verify-release` on the commit to tag.
2. Update `VERSIONS.lock`, crate `version` fields, Lab Kit / Demo pins same week.
3. Update `CHANGELOG.md`.
4. Annotated tag `vX.Y.Z` and `git push origin vX.Y.Z`.
5. Confirm GitHub Release assets (gateway tarballs, offline bundle, Helm, `SHA256SUMS.txt`, SBOM, `VERSIONS.lock`).

Image publish: `.github/workflows/ghcr.yml` → `ghcr.io/synapticfour/ferrum` and `ferrum-ui` (uses `GITHUB_TOKEN` with packages write on the org/repo).

## Continuity

Ferrum is maintained by a single steward (Synaptic Four). To reduce dependence on any one person, the build is documented from a clean machine in this file, the release steps are written down, and the source can be inspected and built without access to Synaptic Four systems (clean-room check dated 2026-10; product crates are not published to crates.io — build from git or use GHCR images). Operational details such as accounts, publication rights and escrow arrangements are being documented in a non-public continuity plan.

## Minimal operator runbook

| Action | Command / note |
|--------|----------------|
| Install (dev proof) | Clone tag → rustup 1.91.1 → `make prove` |
| Install (demo) | Clone → Docker → `make up` or `make up-tes` |
| Start | `make up` / `make up-tes` / `make up-pilot-local` (see Makefile) |
| Stop | `make down` |
| Wipe | `make destroy` |
| Back up | Back up Postgres/MinIO volumes and Crypt4GH key volume; export operator config under `deploy/` / Lab Kit outputs — **exact volume names depend on Compose project** (inspect `docker volume ls` after `make up`) |
| Restore | Restore volumes + config; `make up` |
| Upgrade | Checkout newer tag; compare `CHANGELOG.md` / `VERSIONS.lock`; rebuild or pull GHCR tag; migrate DB per release notes if any |

Detail: `docs/deployment/README.md`, `docs/OPERATOR-TRUST.md`, `docs/INCIDENT_RESPONSE.md`.
