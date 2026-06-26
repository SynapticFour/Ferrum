# Ferrum — Installations- und Betriebshandbuch

Kurzanleitung für IT-Betrieb und Forschungs-IT. Technische Details: [GitHub Releases](https://github.com/SynapticFour/Ferrum/releases).

---

## Voraussetzungen

- Linux-Server (oder Mac für Tests) mit **Docker** und **Docker Compose v2**
- Freie Ports: **8080** (API), **8082** (Weboberfläche) — anpassbar in `.env`
- Ca. **8 GB RAM**, **20 GB Festplatte** für Demo-Stack
- Netzwerk: Internet für Online-Installation; für Air-Gap siehe Offline-Pfad unten

---

## 1. Standard-Installation (Docker Compose)

**Online (empfohlen):**

1. Release von GitHub laden und entpacken, oder Repository klonen.
2. Konfiguration anlegen: `cp deploy/.env.example .env` — Passwörter ändern und **`FERRUM_VERSION=vX.Y.Z`** setzen (Release-Tag, z. B. `v0.2.0`). **Pflicht** — ohne diese Variable bricht `./install.sh` ab; es wird nicht automatisch `:latest` verwendet.
3. `./install.sh` ausführen (startet Stack, prüft `/health`).

**Offline (Air-Gap):**

1. Auf einem Online-Rechner: Release-Artefakt `ferrum-offline-<version>.tar.gz` laden und Prüfsumme mit `SHA256SUMS.txt` verifizieren.
2. Bundle auf den Zielserver kopieren, entpacken, `./import.sh` ausführen.
3. In `.env` **`FERRUM_VERSION`** auf dieselbe Version setzen wie im Bundle, dann `./install.sh --offline` ausführen.

**Ersten Start prüfen:** Browser oder `curl` → `http://<server>:8080/health` muss **200 OK** liefern. Weboberfläche: `http://<server>:8082`.

---

## 2. ga4gh-infra nachinstallieren (Access / Passports)

Ferrum und **ga4gh-infra** (Identität, Passports, Service Registry) werden **getrennt** ausgeliefert. Version in Ferrum `VERSIONS.lock` (`GA4GH_INFRA_REF`) beachten.

1. Passendes **ga4gh-infra-Release** von GitHub laden (Tag laut Ferrum-Dokumentation).
2. `docker/.env.example` nach `docker/.env` kopieren, `GA4GH_INFRA_VERSION` setzen, `./scripts/install.sh` oder `make up` in ga4gh-infra ausführen.
3. In Ferrum `.env`: Broker-URL und JWKS (`KEYCLOAK_JWKS_URL` / `FERRUM_AUTH__*`) auf ga4gh-infra zeigen, Ferrum-Stack neu starten: `docker compose -f deploy/docker-compose.yml up -d`.

Ohne ga4gh-infra läuft Ferrum im Demo-Modus (Keycloak lokal, ohne Clearinghouse).

---

## 3. Installation optional prüfen (HelixTest)

**HelixTest** ist ein separates Validierungstool — nicht Teil des Ferrum-Bundles.

1. HelixTest-Release oder Binary von GitHub laden (empfohlene Version: `HELIXTEST_REF` in Ferrum `VERSIONS.lock`).
2. Ferrum-Stack muss laufen; Basis-URL z. B. `http://localhost:8080`.
3. Beispiel: `helixtest --all --mode ferrum --report table` — grüne Checks bestätigen GA4GH-Kernfunktionen.

---

## Update & Rollback

**Update:** Neue Version in `.env` (`FERRUM_VERSION`, z. B. `v0.2.1`) — **muss gesetzt sein**, sonst verweigert `./install.sh` den Start. Anschließend `./install.sh` oder `docker compose -f deploy/docker-compose.yml up -d`. Details: `docs/deployment/UPDATE-SOP.md`.

**Rollback (2 Schritte):** Vorherige `FERRUM_VERSION` in `.env` setzen → `./install.sh` (oder `docker compose … up -d`). Bei Fehlern vorheriges Offline-Bundle erneut importieren.

**Optional Kubernetes:** Helm-Chart im Release (`ferrum-helm-<tag>.tgz`) — siehe `docs/kubernetes-deployment.md`.

---

**Support:** contact@synapticfour.com · Release-Notes und Prüfsummen auf der GitHub-Release-Seite.
