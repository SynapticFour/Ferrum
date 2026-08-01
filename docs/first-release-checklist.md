# First Release Checklist — Ferrum v0.2.0

Operator checklist before the **first real customer-facing tag** (`v0.2.0`).
Reference: [`portfolio/decisions.md`](./portfolio/decisions.md), [`portfolio/gaps.md`](./portfolio/gaps.md), [`../RELEASING.md`](../RELEASING.md).

> **Phase B note (2026-08-01):** Guided-pilot pack landed — `deploy/configs/pilot.toml` (`require_auth=true`), auth/compute honesty in [`customer-runbook.md`](./customer-runbook.md). Sibling tags `ga4gh-infra-v0.1.0` and HelixTest `v0.1.0` are on `origin`; `VERSIONS.lock` pins `GA4GH_INFRA_REF` / `HELIXTEST_REF` to those tags. Do **not** treat customer install as signed off until the remaining Pre-tag boxes below are checked.

---

## Pre-tag

- [x] **`VERSIONS.lock`:** intended tags named and SHA pins documented (`GA4GH_INFRA_TAG` / `HELIXTEST_TAG` / `*_SHA`)
- [x] **`VERSIONS.lock`:** `GA4GH_INFRA_REF` / `HELIXTEST_REF` set to **real tags** on origin (no bare commit SHAs)
- [ ] **ga4gh-infra:** first tag `ga4gh-infra-v0.1.0` **pushed** to origin (`git push origin ga4gh-infra-v0.1.0`)
- [ ] **HelixTest:** first tag `v0.1.0` **pushed** to origin (`git push origin v0.1.0`)
- [ ] **`CHANGELOG.md`** updated for v0.2.0 cut (Ferrum + ga4gh-infra if applicable) — Unreleased hygiene done; release notes still pending
- [ ] CI green on `main` (Ferrum, ga4gh-infra, pinned clones in CI)

## Guided pilot honesty (Phase B)

- [x] Customer/pilot config with `require_auth=true` (`deploy/configs/pilot.toml`)
- [x] Runbook documents demo `require_auth=false` as **NON-PILOT**; `HELIXTEST_SKIP_AUTH` as CI convenience only
- [x] Runbook documents TES/WES **noop** default, `make up-tes` / `make test-tes`, and pilot compute assumptions
- [x] README softens unsupervised production-compute implication for default demo
- [ ] Optional: scheduled/nightly HelixTest job with auth on (documented; not required on every PR)

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

- [x] **`docs/customer-runbook.md`:** auth honesty, compute honesty, install + ga4gh-infra + HelixTest (fresh-operator dry-run still recommended)
- [x] Runbook covers: online install, ga4gh-infra optional add-on, HelixTest optional verify, version pin / no `:latest`
- [ ] Fresh-operator dry-run: someone **unfamiliar with the product** completes install without asking

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
