# First Release Checklist — Ferrum v0.3.0

Operator checklist before the **next customer-facing tag** (`v0.3.0`). `v0.2.0` (Africa resilience) is already on origin (2026-06-11).
Reference: [`../RELEASING.md`](../RELEASING.md).

> **Phase B note (2026-08-15):** Guided-pilot pack landed — `deploy/configs/pilot.toml` (`require_auth=true`), auth/compute honesty in [`customer-runbook.md`](./customer-runbook.md). Sibling tags on origin: `ga4gh-infra-v0.2.2` (`e43bf08…`) and HelixTest `v0.1.1`. `VERSIONS.lock` pins `GA4GH_INFRA_REF` / `HELIXTEST_REF` to those tags. Do **not** treat customer install as signed off until the remaining Pre-tag boxes below are checked.

---

## Pre-tag

- [x] **`VERSIONS.lock`:** intended tags named and SHA pins documented (`GA4GH_INFRA_TAG` / `HELIXTEST_TAG` / `*_SHA`)
- [x] **`VERSIONS.lock`:** `GA4GH_INFRA_REF` / `HELIXTEST_REF` set to **real tags** on origin (`ga4gh-infra-v0.2.2`, HelixTest `v0.1.1`)
- [x] **ga4gh-infra:** current pin **`ga4gh-infra-v0.2.2`** on origin (annotated tag → `e43bf08bce5132db697a28c7db898d5e18f87b3f`). Do not install `ga4gh-infra-v0.1.0`.
- [x] **HelixTest:** `v0.1.0` on origin; current Ferrum pin is **`v0.1.1`** (htsget region POST → 400 in Ferrum modes)
- [x] **`CHANGELOG.md`** frozen as **`[0.3.0] - 2026-08-15`**. GitHub release notes still pending at tag time (step 5).
- [x] CI green on Ferrum `main` after sibling pins (`ga4gh-infra-v0.2.2`, HelixTest `v0.1.1`).

## Guided pilot honesty (Phase B)

- [x] Customer/pilot config with `require_auth=true` (`deploy/configs/pilot.toml`)
- [x] Runbook documents demo `require_auth=false` as **NON-PILOT**; `HELIXTEST_SKIP_AUTH` as CI convenience only
- [x] Runbook documents TES/WES **noop** default, `make up-tes` / `make test-tes`, and pilot compute assumptions
- [x] README softens unsupervised production-compute implication for default demo
- [ ] Optional: scheduled/nightly HelixTest job with auth on (documented; not required on every PR)

## Release workflow

- [ ] **`release.yml`** run once manually (`workflow_dispatch`) on a release candidate branch or dry-run tag — verify artifacts before `v0.3.0`
- [ ] Tag: `git tag -a v0.3.0 -m "v0.3.0"` && `git push origin v0.3.0`
- [ ] **`SHA256SUMS.txt`** downloaded from GitHub Release — verify locally:
  ```bash
  shasum -a 256 -c SHA256SUMS.txt
  ```

## Install verification (fresh environment)

- [ ] **`install.sh`** tested on a **fresh system** (VM or cloud instance — not your daily dev machine)
- [ ] Copy `.env.example` → `.env`, set `FERRUM_VERSION=v0.3.0`, run `./install.sh`
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

*After Ferrum v0.3.0: repeat a shortened checklist per product (`RELEASING.md` in each repo).*
