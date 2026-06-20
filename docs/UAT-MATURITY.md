# Ferrum UAT maturity tracker

Living checklist for pilot readiness: what is done, what is next, and what remains before **minimum unsupervised tester** maturity.

**Last updated:** 2026-06-14 (Sprint 2 — L2 polish)

---

## Maturity levels

| Level | Meaning |
|-------|---------|
| **L0 — Operator-led only** | Works with hand-holding; misconfig looks like product bugs |
| **L1 — Guided UAT** | Happy path + problem reporting; testers need onboarding doc |
| **L2 — Minimum unsupervised** | Clear errors, noop/auth expectations, import→analysis without wrong workspace |
| **L3 — Production-adjacent** | Chunk hardening, Range preview, auth on all ingest, Fly cold-start UX |

**Current target:** **L2** for local TES + Fly pilot.

---

## Must-do sprint #1 — DONE

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Fix Fly seed docs + `seed-pilot-remote.sh` | Done | Script is verify-only; Fly seed = `./pilot.sh seed all` in `pilot-deploy` |
| 2 | Surface ingest job `error.message` | Done | Poller, banner, `friendlyIngestError` extensions |
| 3 | Auth hardening | Done | AuthCallback failure UI; broker-misconfig banner in AppLayout |
| 4 | Post-submit success UX | Done | `RunSubmitSuccessPanel` in StartAnalysis + SubmitWorkflow dialogs |
| 5 | Noop banner on analysis entry points | Done | Object detail, study setup, analysis dialogs (compact) |
| 6 | Remove `demo-workspace-01` hardcode | Done | `workspace_id` on DRS object API; link-to-workspace gate on object detail |

---

## Sprint 2 — Should-do — DONE

**Goal:** Fewer “silent” failures and operator jargon in empty states.

| # | Area | Task | Status |
|---|------|------|--------|
| 2.1 | Import | Show `max_upload_bytes` in import dialog before upload | Done |
| 2.2 | Import | Block dialog close while upload/job pending | Done |
| 2.3 | DRS | Download/preview errors → `ErrorWithReport` on object detail | Done |
| 2.4 | Data | DataBrowser list errors: auth vs outage + problem report | Done |
| 2.5 | Data | Empty state: tester “Import file” CTA first; operator CLI collapsed | Done |
| 2.6 | Register | Hide “Register by storage path” behind Advanced accordion | Done |
| 2.7 | Workflows | WDL/TRS load failures visible in analysis dialogs | Done |
| 2.8 | Workflows | TRS empty picker guidance + disable Run with hint | Done |
| 2.9 | Workflows | WorkflowCenter list error + problem report | Done |
| 2.10 | Auth | Prominent sign-in banner when `require_auth && !jwt` | Done |
| 2.11 | Reports | Pass HTTP status into `ErrorWithReport.lastApi` | Done |
| 2.12 | Fly UX | UI “warming up” message for cold start / 502 (detect retry) | Done |
| 2.13 | Fly ops | Entrypoint exit non-zero if gateway never healthy | Done |
| 2.14 | Fly ops | Beacon seed failure fails smoke (not warn-only) | Done |
| 2.15 | i18n | de/fr/ar overlays for sprint 2 strings | Done |

---

## Sprint 3 — Backend hardening (target: L2→L3)

**Goal:** Edge cases under load and security alignment.

| # | Area | Task | Priority |
|---|------|------|----------|
| 3.1 | Ingest auth | Apply `ensure_ingest_allowed` to `/upload`, `/upload/chunk`, `/register` | P0 |
| 3.2 | Chunk upload | Validate `total_bytes` against max at session start | P1 |
| 3.3 | Chunk upload | TTL cleanup for orphan `drs/uploads/*` + checkpoints | P1 |
| 3.4 | Chunk upload | Replace O(n²) `append_bytes` for large S3 uploads | P1 |
| 3.5 | Crypt4GH | Fail init hard if keys missing when encrypt expected | P1 |
| 3.6 | Crypt4GH | Streaming encrypt (avoid full-file RAM) for large uploads | P2 |
| 3.7 | DRS stream | Honor `Range` for preview bandwidth | P2 |
| 3.8 | Errors | Map `transfer_queued` (429) in UI with retry hint | P2 |
| 3.9 | Checksum | Surface async checksum failures on object detail | P2 |
| 3.10 | CI | Smoke test chunked upload + failed-job error body | P2 |
| 3.11 | CI | Pilot CI: optional authenticated ingest upload | P2 |

---

## Sprint 4 — Fly / operator (target: unsupervised Fly)

| # | Task | Notes |
|---|------|-------|
| 4.1 | Tester one-pager in UI (welcome / first visit) | Cold start, sign-in, noop WES, import path, report problem |
| 4.2 | Align Fly seed names with local smoke expectations OR document both catalogs | GIAB slice vs “Pilot demo …” |
| 4.3 | Keycloak Fly HTTP health check | Match Ferrum/ga4gh-infra |
| 4.4 | `obtain-passport.sh` hard-fail on invalid JWT | No silent `.env` write |
| 4.5 | Pause vs auto-start documentation in HANDOFF | Testers hitting idle wake |

---

## Known gaps (unchanged — track only)

### Fly pilot fragility

- Cold start 30–90s (`min_machines_running = 0`)
- WES **noop** on Fly — runs complete without container output
- Passport ~3h expiry
- Two seed systems (local `make seed-pilot` vs `pilot.sh seed all`)
- Beacon ingest via `fly ssh` can fail silently (operator seed)

### Local environment

- Docker Desktop Mac: nested TES `docker run` may fail
- `make destroy` wipes Crypt4GH keys volume
- `up-pilot-cloud` requires Fly broker up

### UX (post sprint 1)

- Session errors still English-only in `api/client.ts` throw paths (banner uses i18n)
- Cohort run with 0 samples — weak blocking message
- `?analyze=1` deep link when object load fails

---

## Tester onboarding (minimum — share with cohort)

1. Open **`…/ui/`** (not ga4gh-infra root).
2. **Sign in** when prompted (~3h session on Fly).
3. First load after idle may take **up to 90 seconds** on Fly.
4. Add data via **Import file** → choose file → encrypt (optional) → **Start upload**.
5. **Link object to workspace** before “Use in analysis” if prompted.
6. On Fly: workflows **simulate** runs (noop) — no real GATK output.
7. Errors: use **Report problem** (email / GitHub / copy diagnostics).

---

## Verification commands

| Profile | Command |
|---------|---------|
| Local TES | `make up-tes && make rebuild-gateway-tes && make seed-pilot && make smoke-pilot` |
| Local pilot AAI | `make up-pilot && make test-pilot` |
| Fly verify (after operator seed) | `FERRUM_PASSPORT_JWT=… BASE_URL=https://pasteur-pilot-ferrum.fly.dev ./scripts/seed-pilot-remote.sh` |
| Fly seed (operator) | `cd …/pilot-deploy && ./pilot.sh seed all` |
| UI E2E | `cd services/ui && npm run build && npm run test:e2e` |

---

## Related docs

- [PASTEUR-PILOT.md](PASTEUR-PILOT.md) — Fly URLs and local profiles
- [TEST-COVERAGE-GAPS.md](TEST-COVERAGE-GAPS.md) — CI coverage holes
- [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md) — ingest API notes
- `synapticfour-business/.../pilot-deploy/HANDOFF.md` — operator runbook (private)
