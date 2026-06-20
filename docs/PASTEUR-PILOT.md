# Pasteur Tunis Fly pilot (operator pointer)

Hosted demo for Institut Pasteur de Tunis. **Operator runbooks, Fly URLs, credentials, seed, and troubleshooting** live in the private business repo:

**`synapticfour-business/customers/pasteur-tunis/pilot-deploy/HANDOFF.md`**

This Ferrum doc summarizes what developers need locally.

## Public URLs (default Fly app names)

| Service | URL |
|---------|-----|
| Ferrum UI | https://pasteur-pilot-ferrum.fly.dev/ui/ |
| ga4gh-infra broker | https://pasteur-pilot-ga4gh-infra.fly.dev |
| Login (Keycloak upstream) | https://pasteur-pilot-ga4gh-infra.fly.dev/login/keycloak |
| Keycloak admin | https://pasteur-pilot-keycloak.fly.dev/admin/ |

There is **no web UI on the ga4gh-infra root URL** (API/broker only). End users start at **Ferrum `/ui/`**.

## Local profiles vs Fly

| Goal | Command |
|------|---------|
| Open demo, no AAI | `make up` |
| **AAI locally (mock-idp)** | `make up-pilot` → `make test-pilot` |
| **AAI with Fly Keycloak** | `make up-pilot-cloud` → `make test-pilot-cloud` (Fly must be running) |
| Real workflow compute | `make up-tes` |
| Optional pilot files (local) | `make seed-pilot` after stack is up — BAM, VCF, chr22 ref+truth on MinIO |
| Verify remote Fly (after operator seed) | `FERRUM_PASSPORT_JWT=… BASE_URL=https://pasteur-pilot-ferrum.fly.dev ./scripts/seed-pilot-remote.sh` — verify only |
| **Seed Fly (operator)** | `cd synapticfour-business/customers/pasteur-tunis/pilot-deploy && ./pilot.sh seed all` |
| **Seed name catalogs** | [SEED-CATALOGS.md](SEED-CATALOGS.md) — local `Pilot demo …` vs Fly GIAB-style filenames |

`init-demo.sh` (container init) keeps conformance IDs and honest URL catalog placeholders. **`make seed-pilot`** is optional post-start enrichment: uploads `tiny.bam`, `.bai`, `tiny.vcf`, and a **TinyGermlineHC reference bundle** (`pilot-ref.fa`, truth `.vcf.gz`/`.tbi`) to MinIO, wires `pilot-demo-01` in `demo-cohort-01`, and adds provenance edges. Build fixtures first if missing: `bash profiles/pipeline/fixtures/build-pilot-ref-bundle.sh` and `build-tiny-bam.sh`. Idempotent — safe to re-run. Verify with **`make smoke-pilot`**.

`make up-tes` enables `FERRUM_TES_DOCKER_MOUNT_SOCKET` and passes the host `docker` CLI into cwltool tasks. On **Docker Desktop Mac**, nested `docker run` bind paths may still fail (`host_mnt`); **Linux CI** runs `make test-tes` + `make smoke-pilot` with `SMOKE_REQUIRE_COMPLETE=1` (see CI job `test-tes`). Smoke includes optional **TinyGermlineHC** submit when `SMOKE_GERMLINE=1` (default).

**Crypt4GH:** init generates node keys into the `crypt4gh-keys` volume; gateway mounts them for `encrypt=true` uploads and `/stream` decrypt. Smoke and `make test-tes` verify an encrypt round-trip. Use `docker compose down` (not `-v`) to keep keys with MinIO data — see [CRYPT4GH.md](CRYPT4GH.md#demo-stack-make-up--make-up-tes).

Pilot Fly uses **external auth** (`require_auth=true`, clearinghouse). Built-in Ferrum `/passports/v1` is **disabled** on pilot overlays (see ADR-017 / `docs/GA4GH-INFRA-INTEGRATION.md`).

## After Fly deploy (operator)

```bash
cd synapticfour-business/customers/pasteur-tunis/pilot-deploy
./pilot.sh resume all --wait    # if paused
./scripts/obtain-passport.sh --write-env
./pilot.sh seed all
./scripts/pilot-smoke.sh
```

## Demo expectations

- **Sign-in:** broker → Keycloak demo users (see HANDOFF; rotate before external share).
- **Data:** DRS/Beacon after `./pilot.sh seed all`; workspaces created by seed when `FERRUM_PASSPORT_JWT` is set.
- **WES on Fly:** TES `noop` — API lifecycle only; use `make up-tes` locally for real container runs.
- **Idle wake:** Ferrum may take up to 90s on first load after idle; operator `pause all` stops everything until resume (see HANDOFF “Tester experience”).

## Legal

Ferrum is licensed under **BUSL-1.1** (see root `LICENSE`). The Fly pilot is a **non-production demo**; do not process real patient data. See also `docs/COMPLIANCE.md` and `README.md`.
