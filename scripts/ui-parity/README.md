# Ferrum ui-parity

Customer-acceptance API tests that mirror what the web UI calls (`services/ui`).

## Quick start

```bash
# Local TES stack (read-only — no auth)
make up-tes
make ui-parity-tes

# Local Ferrum + hosted Keycloak (needs Passport JWT and PILOT_CLOUD_GA4GH_URL)
make up-pilot-cloud
export FERRUM_PASSPORT_JWT='…'
make ui-parity-pilot-cloud

# Hosted Ferrum (set URLs; no defaults in-tree)
export FERRUM_URL='https://your-ferrum.example'
export GA4GH_URL='https://your-ga4gh-infra.example'
export FERRUM_PASSPORT_JWT='…'
export PILOT_DIR=/path/to/pilot-deploy   # optional — directory with .env for Fly URLs
make ui-parity-fly
```

## Tiers

| Tier | Meaning |
|------|---------|
| `read` (default) | GET / service-info — safe for customer demos |
| `write` | Creates workspace, submits WES run, delegates deep smoke on `up-tes` |

```bash
make ui-parity-tes UI_PARITY_TIER=write
```

## Profiles

| Profile | Base URL | Auth |
|---------|----------|------|
| `fly` | `$FERRUM_URL` (required) | Passport required |
| `up-tes` | `localhost:8080` | Demo (optional) |
| `up-pilot-cloud` | `localhost:8080` | Passport required |

## Endpoint catalog

`endpoints.json` lists UI-touched routes by feature area. Update it when adding new UI API calls.

## Report

```bash
./scripts/ui-parity/ui-parity.sh --profile fly --report /tmp/ui-parity.md
```

## Relation to other tests

| Script | Role |
|--------|------|
| `smoke-pilot-local.sh` | Deep TES/Crypt4GH/WES (delegated on `up-tes --tier write`) |
| `pilot-smoke.sh` | Fly deploy smoke (delegated on `fly` profile) |
| `verify-source-parity.sh` | Git SHA deploy parity (not functional) |
