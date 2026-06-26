# First Release Checklist — Ferrum v0.2.0

Operator checklist before the **first real customer-facing tag** (`v0.2.0`).  
Reference: [`portfolio/decisions.md`](./portfolio/decisions.md), [`portfolio/gaps.md`](./portfolio/gaps.md), [`../RELEASING.md`](../RELEASING.md).

---

## Pre-tag

- [ ] **`VERSIONS.lock`:** all refs point to **real tags** (no bare commit SHAs)
- [ ] **ga4gh-infra:** first tag `ga4gh-infra-v*.*.*` created and pushed
- [ ] **HelixTest:** first tag created and pushed (pin in Ferrum `VERSIONS.lock`)
- [ ] **`CHANGELOG.md`** updated for v0.2.0 (Ferrum + ga4gh-infra if applicable)
- [ ] CI green on `main` (Ferrum, ga4gh-infra, pinned clones in CI)

## Release workflow

- [ ] **`release.yml`** run once manually (`workflow_dispatch`) on a release candidate branch or dry-run tag — verify artifacts before `v0.2.0`
- [ ] Tag: `git tag -a v0.2.0 -m "v0.2.0"` && `git push origin v0.2.0`
- [ ] **`SHA256SUMS.txt`** downloaded from GitHub Release — verify locally:
  ```bash
  shasum -a 256 -c SHA256SUMS.txt
  ```

## Install verification (fresh environment)

- [ ] **`install.sh`** tested on a **fresh system** (VM or cloud instance — not your daily dev machine)
- [ ] Copy `.env.example` → `.env`, set `FERRUM_VERSION=v0.2.0`, run `./install.sh`
- [ ] Health checks pass (gateway, DRS, or documented endpoints in runbook)
- [ ] **ga4gh-infra** add-on: separate download + install per `VERSIONS.lock` (3-step runbook section)

## Documentation

- [ ] **`docs/customer-runbook.md`:** someone **unfamiliar with the product** can read it and complete install without asking
- [ ] Runbook covers: online install, ga4gh-infra optional add-on, HelixTest optional verify, version pin / no `:latest`

## Offline / air-gap

- [ ] Download `ferrum-offline-*.tar.gz` from release (or build via `export_offline_bundle.sh`)
- [ ] On an **air-gapped** (or network-disabled) host: `./import.sh` → `./install.sh --offline`
- [ ] Same health checks as online path

## Sign-off

- [ ] Release notes on GitHub reviewed
- [ ] Known limitations / hotfix path documented ([`hotfix-process.md`](./hotfix-process.md))
- [ ] Operator name + date: _______________

---

*After Ferrum v0.2.0: repeat a shortened checklist per product (`RELEASING.md` in each repo).*
