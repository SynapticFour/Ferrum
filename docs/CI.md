# CI triggers

Push to `main` and pull requests run only the secret scan and dependency review. Product CI is manual. A `v*` tag publishes the GitHub Release and GHCR images (`full`, `edge`, and `edge-infra`, plus the UI image). The showcase pin is `edge-infra`. A manual GHCR run takes a JSON array in `variants`; the default is `["full","edge","edge-infra"]`.

Run `make verify-release` before tagging. That command is fmt, clippy, workspace tests, the SPDX header check, and `cargo deny check` when `cargo-deny` is installed.

Every workflow has a concurrency group with `cancel-in-progress`. Minutes below are rough runner time, not a measured bill. Green CI is a technical signal, not GA4GH certification. HelixTest conformance workflows are non-pilot.

| Workflow | Trigger | Rough minutes | When to run manually |
|---|---|---|---|
| `ci.yml` | `workflow_dispatch` | 60–150 | Before a tag. Includes workspace tests, UI Playwright, cargo-deny, Helm, ARM, and Docker TES (60-minute timeout). The ARM benchmark job runs when the ref is `main`. |
| `conformance.yml` | `workflow_dispatch` | 30–60 | When you want the non-pilot HelixTest demo lifecycle. |
| `africa-conformance.yml` | `workflow_dispatch` | 30–45 | When you want the Africa HelixTest mode. One job times out at 45 minutes. |
| `mii-conformance.yml` | `workflow_dispatch` | 5–15 | When MII profiles or the connect crate change. |
| `spdx.yml` | `workflow_dispatch` | 1–2 | Optional. `make verify-release` already runs the same check. |
| `codeql.yml` | `workflow_dispatch` | 10–30 | Before a release when you want a CodeQL pass. No weekly schedule. |
| `e2e-real.yml` | `workflow_dispatch` | 20–40 | Real-workflow smoke against the demo stack. |
| `ui-parity.yml` | `workflow_dispatch` | up to 45 | UI acceptance. Pick the profile and tier in the dispatch form. |
| `helixtest-pilot-auth.yml` | `workflow_dispatch` | 30–60 | Auth-on HelixTest. Not certification. |
| `helixtest-ferrum-infra.yml` | `workflow_dispatch` | 30–60 | Ferrum plus ga4gh-infra HelixTest. Not certification. |
| `ghcr.yml` | tag `v*`; `workflow_dispatch` | 20–40 | A tag publishes `full`, `edge`, `edge-infra`, and the UI image. Dispatch to rebuild a subset; default variants include `edge-infra`. |
| `release.yml` | tag `v*`; `workflow_dispatch` | 20–40 | The tag publishes the release. Dispatch is the dry-run. |
| `secret-scan.yml` | pull request; push to `main` or `master` | 1–2 | Leave it. It stays on those events. |
| `dependency-review.yml` | pull request | 1–2 | Leave it. It stays on pull requests. |
