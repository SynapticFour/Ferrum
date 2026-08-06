# Gaps — Customer-Sovereign Deployment

> **HISTORICAL copy** inside Ferrum (Stand **2026-06-26**). See [`README.md`](./README.md).
> SynaptiSec rows obsolete — living product is SynapticProducts (`apps/synaptisec`).
> Referenz: [`audit.md`](./audit.md), [`decisions.md`](./decisions.md).

---

## Zielbild (Reminder)

Jedes Kunden-Release muss:

- Vollständig self-contained sein (Docker + Compose)
- `.env.example` für Kundenkonfiguration enthalten
- `install.sh` haben, das alles in **< 5 Minuten** aufbaut
- **SHA256-Checksums** für Verifikation enthalten
- **Offline-installierbar** sein (Images als `.tar` exportiert)
- Klares **`CHANGELOG.md`** haben

Git: Releases ausschließlich über Tags `v*.*.*` auf `main`. Kein Auto-Update.

*Phase 1 Analyse: 2026-06-26. Phase-2-Abschluss-Review unten.*

---

## Phase 2 — Abschluss-Review

> Gegenstand: Umsetzung Schritte 2–5 laut [`decisions.md`](./decisions.md). Matrix-Spalten beziehen sich auf das **Kategorie-A-Zielbild** (Compose/Offline/install.sh); Kategorie-B-Repos sind **bewusst ausgenommen** wo unten vermerkt.

### Ergebnis nach Repo (Phase-2-Scope)

| Repo | Phase-2-Scope | Status | Lieferumfang (Kurz) | Verbleibende Lücken |
|------|:-------------:|:------:|---------------------|---------------------|
| **Ferrum** | P0 ✅ | **Done** | `VERSIONS.lock`, offline bundle, SHA256, `release.yml`, Runbook, Hotfix | ga4gh-infra weiter separater Download (Entscheidung 1) |
| **ga4gh-infra** | Pin only | **Teilweise** | Ferrum pinnt Tag in `VERSIONS.lock`; eigenes `install.sh` | Kein unified `release.yml`/SHA256/Offline-Bundle auf ga4gh-infra-Ebene (bewusst separater Release-Zyklus) |
| **bioresearch-assistant** | P0 ✅ | **Done** | `install.sh`, prod compose pin, offline + models bundle, `release.yml`, Runbook | `install.py` bleibt Dev-Pfad; Shell-Wrapper ist Prod-Entry |
| **Synaptic-Core** | P1 ✅ | **Done** | `.env.example`, prod compose, `install.sh`, migrations, offline, optional Helm, `release.yml`, Runbook | Dev-Compose noch mit Hardcoded-Creds (absichtlich) |
| **sc-transport** | P1 ✅ | **Done** | Separates Release analog ga4gh-infra; Pin in `VERSIONS.lock` | — |
| **SynaptiSec** | P1 ✅ | **Done** | `deploy.yml` Fly+Vercel, `install.sh` lokal, infra-Dispatch entfernt | Kein Kunden-Offline-Bundle (SaaS) |
| **PCMS** | P1 ✅ | **Done** | `deploy.yml` Vercel, `deployment-runbook.md` | Supabase-Migration manuell (by design); kein Compose |
| **Mycelium** | P1 ✅ | **Done** | `release.yml` APK+Desktop+SHA256, RELEASING, AGPL in Release | Play Store Signing → **Phase 3**; `bootstrap.rs` manuell bei Relay-Deploy |
| **mycelium-relay** | P1 ✅ | **Done** | `deploy.yml` on tag, `MYCELIUM_STORAGE_KEY`-Check | Kein Kunden-Self-Host-Bundle (Fly-SaaS + AGPL-DIY-Doku) |
| **mycelium-web** | P1 ✅ | **Done** | `deploy.yml` on tag | Temporäre Beta-Site (Entscheidung 8) |
| **SecureCollab** | — | **Out of scope** | — | Entscheidung 3: erst nach Security-Audit |
| **Ferrum-Lab-Kit** | — | **Nicht Phase 2** | CLI `install.sh` | Kein Full-Stack-Bundle |
| **HelixTest** | — | **Optional Download** | Pin in Ferrum `VERSIONS.lock` | Nicht im Ferrum-Bundle (Entscheidung 2) |
| **synapticfour-infra** | — | **Minimal C** | — | SynaptiSec entkoppelt; Rest unverändert |

### Gap-Kategorien — Phase-2-Bilanz

| Gap | Vor Phase 2 | Nach Phase 2 | Anmerkung |
|-----|-------------|--------------|-----------|
| **G1 Release-Pipeline** | 🔴 ~15 Repos | 🟢 **Kern-Produkte done** | Ferrum, BRA, Synaptic-Core, sc-transport, Mycelium-Ökosystem, SynaptiSec/PCMS deploy on tag |
| **G2 install.sh** | 🔴 ~10 | 🟢 **Kategorie A done** | BRA, Ferrum, Synaptic-Core, sc-transport, SynaptiSec (lokal); PCMS/Mycelium N/A |
| **G3 .env.example** | 🟠 ~8 | 🟢 **A-Produkte done** | Synaptic-Core, sc-transport ergänzt |
| **G4 Offline `.tar`** | 🔴 ~12 | 🟢 **A-Kern done** | Ferrum, BRA, Synaptic-Core, sc-transport; ga4gh-infra weiter separat |
| **G5 SHA256** | 🟠 ~15 | 🟢 **Release-Artefakte done** | Ferrum, BRA, Synaptic-Core, sc-transport, Mycelium Client |
| **G6 Migrationen** | 🔴 5 | 🟢 **Explizit in install/deploy** | BRA, Synaptic-Core, SynaptiSec; PCMS manuell dokumentiert |
| **G7 Rollback-Doku** | 🟠 ~12 | 🟡 **Teilweise** | Ferrum, BRA, Synaptic-Core Runbooks; PCMS Runbook; ga4gh-infra/Ferrum-Lab-Kit offen |
| **G8 Externe Deps** | 🔴 Prod-Pfade | 🟡 **Verbessert** | SynaptiSec ohne infra-Dispatch; SaaS bleibt Fly/Vercel/Supabase by design |
| **G9 Version Pinning** | 🟠 4 | 🟢 **Ferrum-Ökosystem** | `VERSIONS.lock` Ferrum + Synaptic-Core |
| **G10 CI PR-Gate** | 🟠 | 🟡 **Unverändert** | synapticfour-infra, Showcase — nicht Phase-2-Fokus |
| **G11 CHANGELOG** | 🟠 | 🟡 **Teilweise** | RELEASING.md mehrere Repos; Mycelium/PCMS CHANGELOG-Disziplin weiter manuell |
| **G12 Lizenz/Legal** | Info | 🟡 **AGPL sichtbar** | Mycelium Release + Relay README; BUSL-Kundenverträge weiter organisatorisch |

### Akzeptanzkriterien „Phase 2 Bundle“ (decisions.md Schritte 2–5)

| Bundle / Produkt | Akzeptiert? | Bemerkung |
|------------------|:-----------:|-----------|
| GA4GH Sovereign Stack (Ferrum) | ✅ | Referenz; ga4gh-infra separater Download |
| BioResearch Assistant | ✅ | |
| Synaptic Core + sc-transport | ✅ | Getrennte Releases + Pin |
| SynaptiSec SaaS | ✅ | Fly + Vercel; lokal ohne externe Deps |
| PCMS SaaS | ✅ | Vercel; Supabase manuell |
| Mycelium ecosystem | ✅ | Kein Kunden-Compose; Client+Relay+Beta-Web |

### Bewusst offen → Phase 3+

1. **Play Store Signing** (Mycelium) — debug APK reicht Phase 2
2. **`bootstrap.rs` Sync-Prozess** — dokumentiert (Entscheidung 8); Automatisierung optional
3. **ga4gh-infra** eigenes Offline-Release + SHA256 (falls Air-Gap-Kunden nur AAI-Plane brauchen)
4. **SecureCollab** — Security-Audit + Alembic
5. **Ferrum-Lab-Kit, Showcase, Ferrum-GA4GH-Demo** — kein Kunden-Bundle
6. **synapticfour-business** formales Sovereign-Pack vs Fly-Pilot

### Fazit Phase 2

**Ziel erreicht** für die in `decisions.md` priorisierten Kunden- und SaaS-Produkte. Ferrum bleibt Referenz (~100 % des A-Zielbilds im Kern-Repo). Verbleibende Gaps sind entweder **bewusste Modell-Ausnahmen** (SaaS, Client-Apps) oder **Phase-3/Backlog** — nicht Blocker für erste Kunden-Testzugriffe auf Ferrum/BRA/Synaptic-Core.

---

## Gap-Matrix nach Ziel-Artefakt *(Stand Phase 1 — siehe Abschluss-Review oben für Ist nach Phase 2)*

Legende: ✅ vorhanden · ⚠️ teilweise · ❌ fehlt · N/A nicht anwendbar

| Repo | Compose | `.env.example` | `install.sh` <50Z | SHA256 Release | Offline `.tar` | CHANGELOG | `release.yml` v* | `ci.yml` PR |
|------|:-------:|:--------------:|:-----------------:|:--------------:|:--------------:|:---------:|:----------------:|:-----------:|
| **Ferrum** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **ga4gh-infra** | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ✅ |
| **bioresearch-assistant** | ✅ | ✅ | ⚠️ (`install.py`) | ⚠️ | ✅ | ✅ | ⚠️ | ✅ |
| **Ferrum-Lab-Kit** | ⚠️ generiert | ❌ | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ |
| **Synaptic-Core** | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ GHCR only | ✅ |
| **sc-transport** | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| **SecureCollab** | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| **SynaptiSec** | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ⚠️ |
| **Open-Source-GA4GH-Stack** | ⚠️ generiert | ❌ | ⚠️ PyPI | ❌ | ❌ | ✅ | ⚠️ PyPI | ✅ |
| **HELIOS** | ⚠️ optional | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ PyPI | ✅ |
| **HelixTest** | N/A | N/A | ❌ | ❌ | N/A | ✅ | ❌ | ✅ |
| **Ferrum-GA4GH-Demo** | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ⚠️ |
| **SynapticFour-Showcase** | ⚠️ delegiert | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| **Mycelium** | N/A Client | ❌ | ❌ | ⚠️ APK | N/A | ❌ | ⚠️ manual | ✅ |
| **mycelium-relay** | N/A Fly | ❌ | ❌ | ❌ | N/A | ❌ | ❌ | ✅ |
| **mycelium-web** | N/A static | ✅ | ❌ | N/A | N/A | ❌ | ❌ | ✅ |
| **NeuroAttune** | N/A App | N/A | N/A | N/A | N/A | ❌ | ❌ | ✅ |
| **PCMS** | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| **synapticfour-infra** | ⚠️ | ⚠️ tfvars | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| **synapticfour-business** | ⚠️ pack | ⚠️ pilot | ❌ | ❌ | ⚠️ pack | ❌ | ❌ | ⚠️ |
| **synapticfour-website** | N/A | ✅ | ❌ | N/A | N/A | ❌ | ❌ | ✅ |
| **technical-reports** | N/A | N/A | N/A | N/A | N/A | ❌ | ⚠️ Zenodo | ✅ |
| **ferrum-meta** | N/A | N/A | N/A | N/A | N/A | ✅ | ❌ | ✅ |
| **sc-specs** | N/A | N/A | N/A | N/A | N/A | ✅ | N/A | ✅ |
| **Synaptic-Core-Test** | N/A | N/A | N/A | N/A | N/A | ✅ | N/A | ✅ |

---

## Gap-Kategorien

### G1 — Fehlende Release-Pipeline (Blocker)

**Betroffen:** Synaptic-Core, sc-transport, SecureCollab, SynaptiSec, Ferrum-Lab-Kit, HelixTest, Ferrum-GA4GH-Demo, Showcase, Mycelium, PCMS, synapticfour-*

**Gap:** Kein einheitlicher Workflow „Tag `v*.*.*` → Build Images → Export `.tar` → `SHA256SUMS` → GitHub Release“.

**Ist-Zustand:**
- Ferrum hat `release.yml` + `SHA256SUMS.txt` — **Referenzimplementierung**
- ga4gh-infra: separate Tags pro Komponente (`ga4gh-infra-v*`), kein unified Bundle
- BRA: `build-images.yml` on push, nicht tag-synchron
- Synaptic-Core: GHCR push, aber kein Release-Artefakt-Bundle

**Phase-2-Ziel:** Pro **Kunden-Produkt** ein `scripts/release.sh` + `.github/workflows/release.yml` (nicht zwingend alle 24 Repos).

---

### G2 — Fehlendes `install.sh` (< 5 Min, idempotent)

**Betroffen:** Synaptic-Core, sc-transport, SecureCollab, SynaptiSec, Showcase, PCMS, Ferrum-GA4GH-Demo, Ferrum-Lab-Kit (hat `install.sh` für CLI, nicht Full-Stack)

**Gap:** Kunden müssen `make up`, manuelle Secrets, mehrere Repos kennen.

**Ist-Zustand:**
- Ferrum `install.sh` + `make demo` — gut, aber zwei Einstiege
- ga4gh-infra `scripts/install.sh` — native binary, nicht Full Compose-Stack
- BRA `install.py` — funktional, aber nicht Shell-Standard der Anforderung
- synapticfour-business `self-deployment-pack/` — Dokumentation, kein einzeiliges Script

**Phase-2-Ziel:** Ein Entry-Point pro Produkt-Bundle: `.env` kopieren → `./install.sh` → Health-Check.

---

### G3 — Fehlende / unvollständige `.env.example`

**Betroffen:** Synaptic-Core, sc-transport, Ferrum-Lab-Kit, Ferrum-GA4GH-Demo, Open-Source-GA4GH-Stack, HELIOS, Mycelium, Showcase, synapticfour-infra (nur tfvars)

**Gap:** Kundenvariablen nicht an einem Ort dokumentiert; Compose nutzt Hardcoded-Secrets.

**Kritisch:** Synaptic-Core Compose (`synaptic:synaptic`) ohne `.env.example`.

---

### G4 — Offline-Installierbarkeit (Image `.tar` Export)

**Betroffen:** Synaptic-Core, sc-transport, SecureCollab, SynaptiSec, ga4gh-infra (teilweise), Ferrum-Lab-Kit, Open-Source-GA4GH-Stack, Showcase

**Gap:** Kein standardisierter `docker save` + Bundle-Import im Release.

**Ist-Zustand:**
- Ferrum: `scripts/export_offline_bundle.sh` / `import_offline_bundle.sh` ✅
- BRA: `docs/deployment/OFFLINE-AIRGAP.md` + Scripts ✅
- Rest: Images müssen zur Laufzeit gepullt werden

---

### G5 — SHA256-Checksums im Release

**Betroffen:** ga4gh-infra, BRA, Synaptic-Core, SecureCollab, SynaptiSec, alle ohne Release

**Gap:** Kunden können Artefakte nicht verifizieren.

**Ist-Zustand:** Nur Ferrum Binary-Release liefert `SHA256SUMS.txt` verlässlich.

---

### G6 — Datenbank-Migrationen unsicher

**Betroffen:**

| Repo | Problem |
|------|---------|
| **Synaptic-Core** | Migration-SQL existiert, aber kein Runner; `CREATE TABLE IF NOT EXISTS` ad-hoc |
| **SecureCollab** | Alembic-Ordner leer; inline ALTER |
| **SynaptiSec** | SQL-Migrationen vorhanden, **nicht im Container-Start** |
| **bioresearch-assistant** | Alembic ok, **nicht im Container-Start** — Deploy-Script-Pflicht |
| **PCMS** | Supabase-Migrationen manuell |

**Gap:** Schlechtes Deploy → Schema/App-Mismatch ohne klaren Fehler.

**Phase-2-Ziel:** Migration als expliziter Schritt in `install.sh` oder Entrypoint; Rollback-Doku.

---

### G7 — Kein dokumentierter Rollback

**Betroffen:** ga4gh-infra, Synaptic-Core, sc-transport, SecureCollab, SynaptiSec, Ferrum-Lab-Kit, Showcase, synapticfour-infra, PCMS

**Gap:** Kunde kann Updates nicht sicher rückgängig machen.

**Ist-Zustand:** Nur Ferrum + BRA haben `UPDATE-SOP.md`.

**Phase-2-Ziel:** `docs/customer-runbook.md` Abschnitt Update + Rollback; intern `docs/hotfix-process.md`.

---

### G8 — Externe Deploy-Abhängigkeiten (nicht souverän)

**Betroffen:**

| Abhängigkeit | Repos |
|--------------|-------|
| **synapticfour-infra** + `INFRA_DISPATCH_TOKEN` | SecureCollab, SynaptiSec |
| **Fly.io** (SynapticFour-operated) | synapticfour-business pilot, mycelium-relay |
| **Vercel** | SynaptiSec, PCMS, mycelium-web |
| **Supabase SaaS** | PCMS (Default-Prod) |
| **GitHub API curl install** | Ferrum, ga4gh-infra Default-Pfad |
| **Sibling-Repo auf Disk** | SynapticFour-Showcase, Ferrum-GA4GH-Demo |

**Gap:** Kunde kann nicht vollständig ohne SynapticFour-Infrastruktur deployen (bei Prod-Pfaden).

---

### G9 — Cross-Repo Versions-Pinning

**Betroffen:** Ferrum CI, Pasteur Pilot, Showcase, Ferrum-GA4GH-Demo

**Gap:** `GA4GH_INFRA_REF=main`, `HELIXTEST_REF=main`, `FERRUM_REF=main` — Releases nicht reproduzierbar.

**Phase-2-Ziel:** Release-Bundle pinnt alle Komponenten-Versionen in `VERSIONS.lock` oder Compose-Labels.

---

### G10 — CI/CD Lücken für PR-Gate

**Betroffen:** synapticfour-infra (validate mit `continue-on-error`), Ferrum-GA4GH-Demo (kein Stack-Test), Showcase (nur script smoke)

**Gap:** Grün in CI garantiert nicht deploybare Releases.

**Phase-2-Ziel:** `ci.yml` mit lint + test als PR-Pflicht; Conformance optional nightly.

---

### G11 — CHANGELOG / Release-Notes Disziplin

**Betroffen:** Mycelium, mycelium-relay, mycelium-web, PCMS, synapticfour-*, Showcase, technical-reports

**Gap:** Kunden sehen keine strukturierten Release Notes.

---

### G12 — Lizenz / Legal für Kunden-Deploy

**Betroffen:** BUSL-Repos (Ferrum, Synaptic-Core, sc-transport, BRA), AGPL (Mycelium)

**Gap:** Kunden müssen Lizenzbedingungen vor Self-Deploy klären — nicht technisch, aber Blocker für „maximale Souveränität“.

---

## Priorisierte Gap-Liste (Phase 2)

### P0 — Blocker für erste Kunden-Testzugriffe (GA4GH-Stack)

| # | Gap | Aktion Phase 2 | Repo(s) |
|---|-----|----------------|---------|
| 1 | Kein unified Release-Bundle | `release.sh` + `release.yml` mit `.tar` + SHA256 | Ferrum (+ ga4gh-infra Co-Bundle oder separate Tags dokumentiert) |
| 2 | Offline nicht out-of-the-box | Release-Artefakt enthält `images/*.tar` + `import.sh` | Ferrum |
| 3 | ga4gh-infra SHA256 / CHANGELOG | Root CHANGELOG; SHA256SUMS in `release-binaries.yml` | ga4gh-infra |
| 4 | Version Skew | `VERSIONS.lock` im Release; CI pinnt Tags nicht `main` | Ferrum, ga4gh-infra, Showcase |
| 5 | Kunden-Runbook fehlt | `docs/customer-runbook.md` (1 Seite) | Ferrum (+ ga4gh-infra Abschnitt) |

### P1 — Nächste Kunden-Produkte

| # | Gap | Aktion Phase 2 | Repo(s) |
|---|-----|----------------|---------|
| 6 | Kein `install.sh` | Idempotentes Script <50 Zeilen | bioresearch-assistant (rename/wrapper), SynaptiSec |
| 7 | `:latest` Tags | Compose pinned auf Release-Tag | BRA, SynaptiSec |
| 8 | Migration nicht automatisch | `install.sh` Step `db:migrate` | BRA, SynaptiSec |
| 9 | `.env.example` fehlt | Alle Kundenvariablen | Synaptic-Core, sc-transport |
| 10 | Synaptic-Core Release | GHCR + Compose Bundle + Migration-Runner | Synaptic-Core |

### P2 — Entkopplung von SynapticFour-Infra

| # | Gap | Aktion Phase 2 | Repo(s) |
|---|-----|----------------|---------|
| 11 | Prod braucht infra-Repo | Self-contained Compose-Release ohne dispatch | SecureCollab, SynaptiSec |
| 12 | Fly-Pilot ≠ souverän | self-deployment-pack → formales Release | synapticfour-business |

### P3 — Später / anderes Deploy-Modell

| # | Gap | Repo(s) |
|---|-----|---------|
| 13 | Kein Compose-Release (SaaS) | PCMS — separates Modell oder Self-Host-Dockerfile |
| 14 | Client-Distribution | Mycelium — APK/Desktop Release, kein Compose |
| 15 | Static sites | website, mycelium-web — kein Phase-2-Scope |
| 16 | SecureCollab PoC | Alembic + Security-Audit vor Kunden-Release |

---

## Was Phase 2 **pro Repo** liefern sollte (Vorschlag)

Nicht alle 24 Repos brauchen volle Pipeline. Minimaler Scope:

| Bundle | Repos im Release | Deliverables |
|--------|------------------|--------------|
| **GA4GH Sovereign Stack** | Ferrum + ga4gh-infra (+ optional Ferrum-Lab-Kit) | Compose, `.env.example`, `install.sh`, offline `.tar`, SHA256, CHANGELOG, `release.yml`, `ci.yml`, Runbook |
| **BioResearch Assistant** | bioresearch-assistant | Gleiches Muster; `install.py` → `install.sh` wrapper |
| **Synaptic Core** | Synaptic-Core + sc-transport | Compose, env, Release, Migration-Runner |
| **SynaptiSec Self-Host** | SynaptiSec | Compose + Caddy, migrate in install, von infra entkoppelt |

**Nicht in Phase-2-Scope (ohne explizite Freigabe):**
- PCMS Vercel/Supabase-Pfad
- Mycelium App Store Pipeline
- synapticfour-infra Terraform (Kunde kann Pattern kopieren, aber kein neues Produkt-Bundle)
- technical-reports, website, NeuroAttune

---

## Offene Fragen vor Phase 2

Diese Punkte sollten vor Implementierung geklärt werden:

1. **Welches Produkt bekommen die ersten Testkunden?** (Ferrum-only vs Ferrum+ga4gh-infra vs BRA vs Showcase)
2. **Ein Release-Bundle oder getrennte Releases pro Repo?** (Co-Deploy vs unabhängige Tags)
3. **ga4gh-infra Tag-Schema:** Beibehalten `ga4gh-infra-v*` oder vereinheitlichen auf `v*`?
4. **BUSL-Kunden:** Gibt es bereits Lizenzvereinbarungen für Self-Host?
5. **HelixTest im Release-Bundle?** (Als Validierungstool mitliefern vs separater Download)
6. **Root-Docs vs Repo-Docs:** Phase-2-Dateien im Ferrum-Repo oder Meta-Repo / Workspace-Root?

---

## Zusammenfassung

| Kategorie | Anzahl Repos betroffen | Schwere |
|-----------|------------------------|---------|
| G1 Release-Pipeline | ~15 | 🔴 Kritisch |
| G2 install.sh | ~10 | 🔴 Kritisch |
| G3 .env.example | ~8 | 🟠 Hoch |
| G4 Offline `.tar` | ~12 | 🔴 Kritisch (Air-gap Kunden) |
| G5 SHA256 | ~15 | 🟠 Hoch |
| G6 Migrationen | 5 | 🔴 Kritisch |
| G7 Rollback-Doku | ~12 | 🟠 Hoch |
| G8 Externe Deps | 6 | 🔴 Kritisch (Prod-Pfade) |
| G9 Version Pinning | 4 | 🟠 Hoch |

**Fazit:** Ferrum ist die Referenz (~80 % des Zielbilds). ga4gh-infra und bioresearch-assistant folgen dicht. Der Rest des Portfolios braucht entweder **Phase-2-Bundle** oder bewusste **Ausnahme** vom Compose-Release-Modell.

---

*Erstellt Phase 1. Phase-2-Abschluss-Review: 2026-06-26.*
