# Test Coverage Gaps

This document tracks what Ferrum validates in **Rust unit/integration tests** vs **HelixTest** (`--mode ferrum` standard CI and `--mode ferrum-africa` opt-in profiles). Use it when planning conformance expansion or demo-stack hardening.

See also: [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md), [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md).

---

## Summary matrix

| Feature area | Rust tests | HelixTest `--mode ferrum` | HelixTest `--mode ferrum-africa` |
|--------------|------------|---------------------------|----------------------------------|
| GA4GH core (DRS, WES, TES, TRS, Beacon human, htsget) | Yes | Yes (`--all`) | Partial (offline profile: health + service-info) |
| ONT ingest + pathogen Beacon | Yes | No (by design) | **ont** profile |
| Outbreak mode | Yes | No | **outbreak** profile (skips unless `[outbreak] enabled`) |
| Federation (`federate=true`) | Yes | No | **federation** profile (needs `FERRUM_AFRICA_PEER_URL` + 2nd gateway) |
| Reference genome registry | Yes | No | **offline** profile |
| WES `REFERENCE_MISMATCH` warning | Yes | No | **Not covered** |
| Bandwidth monitor / transfer queue / resume tokens | Yes | No | **Not covered** |
| Power / solar mode / emergency checkpoint | Yes | No | **Not covered** |
| Data residency audit chain | Yes | No | Partial (outbreak profile verifies when entries exist) |
| Chunked upload / resume | Yes | No | **Not covered** |
| Laptop Mode (SQLite embed) | Yes | No (Docker stack uses Postgres) | No |

**Legend:** “No (by design)” = intentionally out of scope for standard GA4GH conformance; Africa features are opt-in via `ferrum-africa`.

---

## Docker demo stack gaps

| Gap | Impact | Mitigation / follow-up |
|-----|--------|------------------------|
| **Partial DB volumes** | `ferrum-init` could fail re-run when later migrations use plain `CREATE TABLE` on existing relations | **Fixed:** `deploy/scripts/init-demo.sh` tracks `_ferrum_init_migrations`, bootstraps from `_sqlx_migrations` or existing schema; only applies pending `.up.sql`. Fresh reset: `docker compose down -v`. |
| **Gateway migrations disabled** | Demo sets `FERRUM_DATABASE__RUN_MIGRATIONS=false`; schema comes from init only | Documented in compose; do not rely on gateway auto-migrate in demo. |
| **Outbreak disabled in default demo config** | Africa **outbreak** HelixTest profile skips (404 on activate) | Enable `[outbreak]` in demo TOML or dedicated CI config for full outbreak E2E. |
| **Federation needs two gateways** | **federation** profile skips without peer | Set `FERRUM_AFRICA_PEER_URL`; run second gateway instance in CI or manual test. |
| **HelixTest Africa CI clones `main`** | Ferrum `africa-conformance.yml` uses upstream HelixTest; local HelixTest fixes must be **pushed** before CI picks them up | Push HelixTest repo after Africa fixture/metadata fixes. |
| **Auth Level 4** | Strict JWT-on-every-request tests conflict with demo auth layout | CI sets `HELIXTEST_SKIP_AUTH=true` for `--mode ferrum`; see [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md). |

---

## Recommended follow-ups (priority)

1. **HelixTest Africa:** add cases for WES `reference_genome` + `REFERENCE_MISMATCH`, bandwidth/resume, and power-mode headers (Rust coverage exists; HTTP E2E does not).
2. **Demo config:** ship `[outbreak] enabled = true` variant or CI overlay so outbreak profile does not skip by default in Africa workflow.
3. **Federation CI:** optional second-gateway job matrix for `federation` profile (heavier; keep skippable locally).
4. **Standard conformance:** keep `--mode ferrum --all` unchanged; Africa remains opt-in per ADR-016.

---

## Where tests live

| Layer | Location |
|-------|----------|
| Rust workspace | `cargo test --workspace` (crate `tests/` and `#[cfg(test)]` modules) |
| Standard HelixTest | `.github/workflows/conformance.yml`, `deploy/scripts/run-helixtest-local.sh --full` |
| Africa HelixTest | `.github/workflows/africa-conformance.yml`, `helixtest --mode ferrum-africa --africa-profile …` |
| Laptop embed E2E | `deploy/scripts/ci-laptop-demo-e2e.sh`, job `test-laptop-mode` |
