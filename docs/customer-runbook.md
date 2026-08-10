# Ferrum — Installations- und Betriebshandbuch

Kurzanleitung für IT-Betrieb und Forschungs-IT. Technische Details: [GitHub Releases](https://github.com/SynapticFour/Ferrum/releases).

---

## Voraussetzungen

- Linux-Server (oder Mac für Tests) mit **Docker** und **Docker Compose v2**
- Freie Ports: **8080** (API), **8082** (Weboberfläche) — anpassbar in `.env`
- Ca. **8 GB RAM**, **20 GB Festplatte** für Demo-Stack
- Netzwerk: Internet für Online-Installation; für Air-Gap siehe Offline-Pfad unten

---

## Auth honesty (pilot vs demo / CI)

| Profile | Config | `require_auth` | Use for |
|---------|--------|----------------|---------|
| **Demo / local** | `deploy/configs/local.toml`, default `make up` | `false` | Dev and quick demos only — **NON-PILOT** |
| **Guided pilot** | `deploy/configs/pilot.toml` | `true` | Customer pilots; wire JWKS to ga4gh-infra / IdP |
| **Production** | `deploy/configs/production.toml` | `true` | Hardened deploy |

**Do not treat demo defaults as pilot evidence.** A stack with `require_auth=false` is intentionally open for local exploration; it is not a customer auth posture.

**WES fail-closed (H2):** When `require_auth=true` / `FERRUM_AUTH__REQUIRE_AUTH=true`, unauthenticated `GET/POST /ga4gh/wes/v1/runs` (and cancel/resume/status/log/tasks) return **HTTP 401**. Service-info remains public. Ingest still requires a Bearer **and** `ferrum:collector` (or admin) — mock-idp Passports alone are not enough; issue a collector visa via IdP or Edge local accounts ([INSTALLATION.md](./INSTALLATION.md) visa table).

**Solum consent teeth (H2.1):** When `[solum]` / `FERRUM_SOLUM__BASE_URL` (+ sidecar token) is set, Ferrum polls Solum `GET /v1/consent/status` for bound DRS byte access and WES `POST /runs`. Binding comes from object metadata / run tags `solum_subject` + `solum_purpose`, or `FERRUM_SOLUM__DEFAULT_SUBJECT` / `DEFAULT_PURPOSE`. Only `status=granted` allows; revoke / unknown / Solum down → **HTTP 403**. Unset `BASE_URL` leaves behaviour unchanged. Contract: Showcase ADR 0001.

**Solum subject bridge (H3.3):** Prefer the same string for Ferrum `solum_subject` metadata and Solum `solum_subject_id` in `POST /v1/cdr/subject-link` (optional `ferrum_drs_id` = DRS object id). Creating `POST /v1/fhir/Patient` on Solum auto-upserts a subject-link with `solum_subject_id = Patient.id` — use that id as DRS `solum_subject`. Contract: Solum [ADR 0003](https://github.com/SynapticFour/Solum/blob/main/docs/adr/0003-subject-bridge.md).

**Managed single-tenant (H5 optional):** A hosted Ferrum install for one customer is one deployment (config, storage, secrets) — not a shared multi-tenant DRS/WES schema. Align with Showcase [ADR 0003 tenant boundaries](https://github.com/SynapticFour/SynapticFour-Showcase/blob/main/docs/adr/0003-tenant-boundaries.md). Workspace / object ACLs inside one deployment remain Ferrum’s existing model; do not invent cross-customer row tenancy in H5.
**Pilot-local issuer:** Tokens from `http://localhost:8180/login/…` carry `iss=http://localhost:8180`. `deploy/docker-compose.pilot.yml` sets `FERRUM_AUTH__ISSUER` to that public URL while fetching JWKS from `http://aai-broker:8080` in-cluster. Mismatching issuer/JWKS trust causes silent auth failure (Bearer present but treated as anonymous → 401 on WES).

**HelixTest / conformance CI:** the Conformance workflow sets `HELIXTEST_SKIP_AUTH=true` so Auth (Level 4) is skipped against the demo stack. That is a **CI convenience**, not customer or compliance evidence. For pilot verification, run HelixTest (or your own checks) against a stack using `deploy/configs/pilot.toml` (or equivalent) with `require_auth=true` and a real JWKS, and **do not** set `HELIXTEST_SKIP_AUTH`. Details: [HELIXTEST-INTEGRATION.md](./HELIXTEST-INTEGRATION.md).

**Nightly / scheduled auth-on path (recommended, not required on every PR):** run a separate job or cron that starts the stack with `FERRUM_AUTH__REQUIRE_AUTH=true` (or `FERRUM_CONFIG=deploy/configs/pilot.toml`), unset `HELIXTEST_SKIP_AUTH`, and execute HelixTest Auth Level 4. Prefer this over enabling auth on every PR conformance job.

---

## Compute honesty (TES / WES noop)

**Default demo (`make up`, default Compose):** TES uses the **noop** backend. WES run lifecycle APIs work, but tasks do **not** execute real containers or produce real workflow outputs. UI may show completed-looking runs from seeds or simulated states.

**What pilots must not assume**

- That a green WES/TES API or HelixTest result on the default demo means unsupervised production compute.
- That Fly / hosted pilot overlays with TES `noop` run real GATK or container workloads.
- That conformance CI (also noop-aligned for TES checksums) proves site compute capacity.

**How to enable real TES locally**

```bash
make up-tes          # demo + Docker-backed TES
make test-tes        # ingest / Crypt4GH / WES COMPLETE smoke
make smoke-pilot     # broader pilot smoke (after optional make seed-pilot)
```

See [TESTING.md](../TESTING.md) and [TES-DOCKER-BACKEND.md](./TES-DOCKER-BACKEND.md). Nested Docker on macOS Desktop can still fail bind mounts; Linux CI job `test-tes` is the hard gate for container-backed paths.

---

## 1. Standard-Installation (Docker Compose)

**Online (empfohlen):**

1. Release von GitHub laden und entpacken, oder Repository klonen.
2. Konfiguration anlegen: `cp deploy/.env.example .env` — Passwörter ändern und **`FERRUM_VERSION=vX.Y.Z`** setzen (Release-Tag, z. B. `v0.2.0`). **Pflicht** — ohne diese Variable bricht `./install.sh` ab; es wird nicht automatisch `:latest` verwendet.
3. Für **Pilot**: Auth-Profil mit `require_auth=true` verwenden (`deploy/configs/pilot.toml` oder `production.toml`) und JWKS setzen — nicht die Demo-Defaults.
4. `./install.sh` ausführen (startet Stack, prüft `/health`).

**Offline (Air-Gap):**

1. Auf einem Online-Rechner: Release-Artefakt `ferrum-offline-<version>.tar.gz` laden und Prüfsumme mit `SHA256SUMS.txt` verifizieren.
2. Bundle auf den Zielserver kopieren, entpacken, `./import.sh` ausführen.
3. In `.env` **`FERRUM_VERSION`** auf dieselbe Version setzen wie im Bundle, dann `./install.sh --offline` ausführen.

**Ersten Start prüfen:** Browser oder `curl` → `http://<server>:8080/health` muss **200 OK** liefern. Weboberfläche: `http://<server>:8082`.

---

## 2. ga4gh-infra nachinstallieren (Access / Passports)

Ferrum und **ga4gh-infra** (Identität, Passports, Service Registry) werden **getrennt** ausgeliefert. Version in Ferrum `VERSIONS.lock` (`GA4GH_INFRA_REF` / intended `GA4GH_INFRA_TAG`) beachten.

1. Passendes **ga4gh-infra-Release** von GitHub laden (Tag laut Ferrum-Dokumentation — prefer `ga4gh-infra-v*` once published; until then the pin may be a commit SHA documented in `VERSIONS.lock`).
2. `docker/.env.example` nach `docker/.env` kopieren, `GA4GH_INFRA_VERSION` setzen, `./scripts/install.sh` oder `make up` in ga4gh-infra ausführen.
3. In Ferrum `.env`: Broker-URL und JWKS (`KEYCLOAK_JWKS_URL` / `FERRUM_AUTH__*`) auf ga4gh-infra zeigen, Ferrum-Stack neu starten: `docker compose -f deploy/docker-compose.yml up -d`.

Ohne ga4gh-infra läuft Ferrum im Demo-Modus (Keycloak lokal, ohne Clearinghouse) — geeignet für Entwicklung, nicht als Pilot-Auth-Nachweis.

---

## 3. Installation optional prüfen (HelixTest)

**HelixTest** ist ein separates Validierungstool — nicht Teil des Ferrum-Bundles.

1. HelixTest-Release oder Binary von GitHub laden (empfohlene Version: `HELIXTEST_REF` / intended `HELIXTEST_TAG` in Ferrum `VERSIONS.lock`).
2. Ferrum-Stack muss laufen; Basis-URL z. B. `http://localhost:8080`.
3. Beispiel: `helixtest --all --mode ferrum --report table` — grüne Checks bestätigen GA4GH-Kernfunktionen.
4. Für **Pilot-Auth-Nachweis**: Stack mit `require_auth=true`, **ohne** `HELIXTEST_SKIP_AUTH`.

---

## Update & Rollback

**Update:** Neue Version in `.env` (`FERRUM_VERSION`, z. B. `v0.2.1`) — **muss gesetzt sein**, sonst verweigert `./install.sh` den Start. Anschließend `./install.sh` oder `docker compose -f deploy/docker-compose.yml up -d`. Details: `docs/deployment/UPDATE-SOP.md`.

**Rollback (2 Schritte):** Vorherige `FERRUM_VERSION` in `.env` setzen → `./install.sh` (oder `docker compose … up -d`). Bei Fehlern vorheriges Offline-Bundle erneut importieren.

**Optional Kubernetes:** Helm-Chart im Release (`ferrum-helm-<tag>.tgz`) — siehe `docs/kubernetes-deployment.md`.

---

**Support:** contact@synapticfour.com · Release-Notes und Prüfsummen auf der GitHub-Release-Seite.
