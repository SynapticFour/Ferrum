# Phase-2-Entscheidungen — Deployment-Portfolio

> **HISTORICAL copy** inside Ferrum (Stand **2026-06-26**). See [`README.md`](./README.md).
> **Entscheidung 4 (SynaptiSec)** superseded — Products-only; standalone archived.
> Referenz: [`audit.md`](./audit.md), [`gaps.md`](./gaps.md).
> Referenzimplementierung (damals): **Ferrum** (~80 % Zielbild).

---

## Portfolio-Kategorien (final)

### Kategorie A — Customer-Sovereign

Kunden deployen selbst. Alle drei Pfade müssen funktionieren:

1. **Online:** GitHub Releases → `./install.sh`
2. **Offline/Air-Gap:** `.tar`-Bundle → `./import.sh` → `./install.sh`
3. **Lokal/Dev:** `docker compose up` oder `make dev`

| Repo | Rolle im Bundle |
|------|-----------------|
| Ferrum | GA4GH Data/Compute Plane (Kern) |
| ga4gh-infra | Identity/Access Plane (Teil des Ferrum-Ökosystems) |
| Ferrum-Lab-Kit | Compose-Generator für selektive GA4GH-Services |
| Synaptic-Core | Plattform-Server |
| sc-transport | QUIC/SSE-Transport |
| bioresearch-assistant | On-Prem Biomedical Research Platform |

### Kategorie B — SaaS/Service

SynapticFour deployt, Kunden nutzen. Zwei Pfade:

1. **Prod:** GitHub Actions bei Tag (`v*.*.*`)
2. **Lokal:** `docker compose up` — Prod-Parity soweit möglich

| Repo | Prod-Ziel |
|------|-----------|
| SynaptiSec | Fly.io (Backend) + Vercel (Frontend) |
| PCMS | Vercel (bleibt) |
| NeuroAttune | App Store / sideload (kein Compose-Release) |
| **Mycelium** | **Play Store + Desktop App** (langfristig); GitHub Release APK/Desktop on tag (Phase 2) |
| mycelium-relay | Fly.io on tag (SynapticFour Bootstrap-Relays) |
| mycelium-web | Vercel on tag (**temporäre Beta-Site**, kein dauerhafter Infra-Bestandteil) |

### Kategorie C — Intern (minimaler Scope)

Keine Phase-2-Arbeit außer Abhängigkeiten für A/B.

`synapticfour-infra`, `synapticfour-business`, `synapticfour-website`, `technical-reports`, `ferrum-meta`, `sc-specs`, `Synaptic-Core-Test`, `SynapticFour-Showcase`, `HelixTest`, `Ferrum-GA4GH-Demo`, `Open-Source-GA4GH-Stack`, `HELIOS`

---

## Entscheidung 1 — ga4gh-infra Bundle-Strategie

**Kontext:** ga4gh-infra hat eigenes `deploy/docker-compose.yml` und Tag-Schema `ga4gh-infra-v*`. Ferrum und ga4gh-infra werden typischerweise co-deployed (Pasteur-Pilot, `docker-compose.ga4gh-infra.yml`).

| Option | Beschreibung |
|--------|--------------|
| **A** | ga4gh-infra-Images direkt in Ferrum `release.yml` integrieren — **ein Bundle, ein `install.sh`** |
| **B** | ga4gh-infra bleibt separater Download; Ferrum referenziert Version in `VERSIONS.lock` |

**Entscheidung:** ✅ **Option B** — ga4gh-infra bleibt separater Download; Ferrum referenziert die kompatible Version in `VERSIONS.lock`.

**Begründung:** ga4gh-infra behält eigenes Tag-Schema (`ga4gh-infra-v*`) und unabhängige Release-Zyklen. Ferrum-Bundle bleibt fokussiert auf die Data/Compute Plane; AAI-Plane wird explizit nachinstalliert.

**Konsequenzen:**

- `VERSIONS.lock` (oder `deploy/VERSIONS.env`) enthält `GA4GH_INFRA_VERSION=<ga4gh-infra-vX.Y.Z>`
- Ferrum `release.yml` baut **keine** ga4gh-infra-Images mit
- Offline-Bundle: Ferrum `.tar` + separater ga4gh-infra `.tar` (jeweils mit SHA256)
- `install.sh` / `docs/customer-runbook.md`: Abschnitt „ga4gh-infra installieren“ (3 Schritte)
- CI pinnt `GA4GH_INFRA_REF` auf Tag aus `VERSIONS.lock`, nicht `main`

---

## Entscheidung 2 — HelixTest im Kunden-Bundle

**Kontext:** HelixTest ist ein Rust-CLI-Validierungstool (kein Server). Kunden könnten damit ihre Installation selbst verifizieren. Ferrum CI nutzt HelixTest bereits gegen live Stacks.

| Option | Beschreibung |
|--------|--------------|
| **A** | Binary im Ferrum-Release-Bundle mitliefern |
| **B** | Separater optionaler Download; im Runbook erwähnt |

**Entscheidung:** ✅ **Option B** — Separater optionaler Download; Verifikation im Runbook dokumentiert.

**Begründung:** HelixTest ist Validierungstool, kein Runtime-Service. Kern-Bundle bleibt schlank; Kunden, die Conformance brauchen, laden HelixTest gezielt nach.

**Konsequenzen:**

- Kein HelixTest-Binary im Ferrum-Release-Artefakt
- `VERSIONS.lock` enthält `HELIXTEST_VERSION=<tag>` als Empfehlung (nicht Bundle-Pflicht)
- `docs/customer-runbook.md`: Abschnitt „Installation verifizieren (optional)“ mit Download-Link + Beispielkommando
- Ferrum CI pinnt `HELIXTEST_REF` auf Tag aus `VERSIONS.lock`

---

## Entscheidung 3 — SecureCollab: bewusst aus Phase 2

**Status:** ✅ **Entschieden — kein Phase-2-Scope**

**Kontext (Audit):**

- Expliziter PoC-Status, nicht production-ready
- Default `SECRET_KEY` in `.env.example`
- Alembic-Ordner leer; Runtime nutzt `create_db_and_tables()` + inline ALTER
- Prod-Pfad hängt an `synapticfour-infra` Dispatch

**Entscheidung:** SecureCollab kommt **nicht** in Phase 2. Erst nach explizitem Security-Audit und vollständigem Alembic-Setup.

**Konsequenzen:** Keine `install.sh`, kein `release.yml`, keine Entkopplung von synapticfour-infra in Phase 2. Kategorie C bis Freigabe.

---

## Entscheidung 4 — SynaptiSec Prod-Pfad (Kategorie B)

**Kontext:** SynaptiSec ist SaaS — SynapticFour deployt, Kunden nutzen. Heute existieren parallele Pfade: Vercel, `ghcr-infra.yml` → Hetzner via synapticfour-infra. Migration-Runner (`scripts/migrate.mjs`) ist **nicht** im Container.

**Phase-2-Ziel:**

- Ein sauberer Prod-Pfad (kein infra-Repo-Dispatch)
- Migration als expliziter Schritt im Deploy-Script
- Lokal: `docker compose up` ohne externe Deps

| Option | Beschreibung |
|--------|--------------|
| **Fly.io** | Backend on Fly.io + Vercel Frontend; `deploy.yml` on tag `v*.*.*` |
| **Hetzner direkt** | Deploy-Script im SynaptiSec-Repo; keine synapticfour-infra-Dispatch-Kette |

**Entscheidung:** ✅ **Fly.io** (Backend) + **Vercel** (Frontend) — kein synapticfour-infra-Dispatch.

**Begründung:** Konsistent mit anderen Kategorie-B-Services (mycelium-relay, business-Pilot); weniger VM-Ops als Hetzner-IaC; Vercel-Pfad existiert bereits.

**Konsequenzen:**

- Neues `deploy.yml` (oder angepasstes Workflow) on tag `v*.*.*` → Fly.io Backend
- Vercel-Deploy on tag (parallel oder im selben Workflow)
- Migration: expliziter Schritt `npm run db:migrate` / `scripts/migrate.mjs` **vor** oder **nach** Image-Deploy — nicht im Container-CMD
- `ghcr-infra.yml` → synapticfour-infra Dispatch wird deprecated/entfernt in Phase 2
- Lokal: `docker compose up` bleibt unverändert funktionsfähig (Prod-Parity soweit möglich)

---

## Entscheidung 5 — Synaptic-Core + sc-transport Bundle

**Status:** ✅ **Entschieden — getrennte Releases**

**Entscheidung:** Synaptic-Core und sc-transport haben **separate Tags** und **`release.yml`** Workflows — analog zu Ferrum und ga4gh-infra.

**Konsequenzen:**

- Kein gemeinsames Offline-Bundle
- `Synaptic-Core/VERSIONS.lock` referenziert kompatible **`SC_TRANSPORT_VERSION`** (Dokumentation/Pin, separater Download)
- Kunden installieren jedes Produkt mit eigenem `./install.sh`

---

## Entscheidung 6 — Synaptic-Core Helm (optional)

**Status:** ✅ **Optional `deploy/helm/` anlegen**

**Entscheidung:** Minimaler Helm-Chart nach Ferrum-Vorlage; versioniert mit Synaptic-Core-Tag; **nicht** Phase-2-Blocker (Compose bleibt Standard).

**Konsequenzen:** `deploy/helm/values.yaml.example`, `docs/kubernetes-deployment.md`; Chart im Release-Artefakt `synaptic-core-helm-<tag>.tgz`.

---

## Entscheidung 7 — Synaptic-Core Versionsvariable

**Entscheidung:** **`SYNAPTIC_CORE_VERSION`** in `.env` / `install.sh` (explizit, nicht `SC_VERSION`).

**sc-transport:** **`SCT_VERSION`** in `.env` / `install.sh`.

---

## Entscheidung 8 — Mycelium Ecosystem (Kategorie B)

**Status:** ✅ **Phase 2 umgesetzt** (Client-Release, Relay-Deploy, Beta-Web)

**Produktmodell:**

| Komponente | Rolle | Phase 2 | Langfristig |
|------------|-------|---------|-------------|
| **Mycelium (Client)** | Mesh-Messenger-App | GitHub Release: APK (debug) + Tauri Desktop + SHA256SUMS on tag | **Play Store + Desktop App** — **keine self-hosted Infrastruktur für Kunden** |
| **mycelium-relay** | Bootstrap-Relay (libp2p) | Fly.io on tag; SynapticFour betreibt globale Relays | Community darf eigene Relays hosten (**AGPL-3.0-or-later**) |
| **mycelium-web** | Marketing/Beta-Landing | Vercel on tag | **Temporäre Beta-Site** — kein dauerhafter Infrastruktur-Bestandteil |

**Relay / Bootstrap (operativ wichtig):**

- SynapticFour betreibt globale Bootstrap-Relays (z. B. `mycelium-relay.fly.dev`).
- Der Client enthält **hardcoded Relay-Adressen** in `Mycelium/crates/mycelium-core/src/bootstrap.rs` (`RELAY_IPV4`, `RELAY_PEER_ID`, `BOOTSTRAP_PEERS`).
- **Bei jedem neuen Relay-Deploy** (neue Peer-ID oder dedizierte IPv4): `bootstrap.rs` **und** Client-Release aktualisieren — sonst verlieren Clients Bootstrap.
- Fly-App **muss** `MYCELIUM_STORAGE_KEY` als Secret haben (stabile Peer-ID über Deploys).

**Play Store Signing:** **Phase 3** — nicht Phase 2. Phase-2-Releases liefern debug-signiertes APK zum Sideload.

**AGPL:** Release-Notes und README weisen auf Netzwerk-Service-Pflichten hin (modifizierter Relay/App).

---

## Implementierungsreihenfolge (nach Bestätigung Schritt 1)

| Schritt | Scope | Abhängigkeit |
|---------|-------|--------------|
| 2 | P0 Ferrum (2a–2d) | ✅ Entscheidungen 1 + 2 bestätigt |
| 3 | P0 bioresearch-assistant (3a–3d) | Ferrum bestätigt |
| 4 | P1 Synaptic-Core Ecosystem (4a–4c) | ✅ Entscheidung 5 + 6 + 7 |
| 5 | P1 SaaS + Mycelium ecosystem | ✅ abgeschlossen |

**Phase 2 Implementierung:** abgeschlossen (2026-06-26). Abschluss-Review: [`gaps.md` § Phase-2-Abschluss](./gaps.md#phase-2--abschluss-review).

---

## Referenz-Patterns (Ferrum — nicht neu erfinden)

| Artefakt | Ferrum-Pfad |
|----------|-------------|
| Compose | `deploy/docker-compose.yml` |
| Env-Vorlage | `deploy/.env.example` |
| Install | `install.sh` |
| Offline | `scripts/export_offline_bundle.sh`, `import_offline_bundle.sh` |
| Release CI | `.github/workflows/release.yml` |
| Rollback | `docs/deployment/UPDATE-SOP.md` |

---

*Letzte Aktualisierung: 2026-06-26 — Phase 2 abgeschlossen (Schritte 2–5). Mycelium: Entscheidung 8.*
