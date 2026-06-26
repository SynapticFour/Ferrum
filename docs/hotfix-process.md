# Hotfix-Prozess (intern)

> Kurz-SOP für sicherheitsrelevante oder produktionsblockierende Fixes zwischen regulären Releases.
> Kunden-Installation: [customer-runbook.md](./customer-runbook.md)

---

## 1. Hotfix-Branch vom letzten Tag

```bash
LAST_TAG=v0.2.0   # letzter stabiler Release
git fetch --tags
git checkout -b hotfix/0.2.1 "${LAST_TAG}"
```

Nur den minimalen Fix committen — keine Feature-Arbeit.

---

## 2. Fix nach `main` zurückmergen

```bash
git checkout main
git pull
git merge --no-ff hotfix/0.2.1 -m "Merge hotfix/0.2.1"
```

`VERSIONS.lock` prüfen (Pins nur ändern, wenn der Fix Abhängigkeiten betrifft).

CI grün: build, lint, conformance, release-Workflow (Dry-Run optional via Workflow-Dispatch).

---

## 3. Patch-Tag und Release

```bash
# CHANGELOG.md: Eintrag unter [0.2.1]
git tag -a v0.2.1 -m "v0.2.1 — hotfix: <Kurzbeschreibung>"
git push origin main v0.2.1
```

Release-Workflow erzeugt Artefakte inkl. `SHA256SUMS.txt`. Vor Freigabe Prüfsummen und Smoke-Test (`./import.sh` + `./install.sh --offline` mit gesetztem `FERRUM_VERSION`).

**Wichtig:** `./install.sh` bricht ab, wenn `FERRUM_VERSION` in `.env` fehlt — kein stilles `:latest`. Hotfix-Release-Notes müssen die exakte Version nennen.

---

## 4. Kunden informieren (Vorlage)

**Betreff:** Ferrum Hotfix v0.2.1 — empfohlenes Update

```
Guten Tag,

wir haben Ferrum v0.2.1 veröffentlicht (Hotfix für: <Problem in einem Satz>).

Empfohlene Schritte:
1. Release und SHA256SUMS.txt von GitHub laden und verifizieren
2. In .env setzen: FERRUM_VERSION=v0.2.1  (Pflicht — install.sh startet sonst nicht)
3. Images importieren/starten: ./import.sh  (offline) oder ./install.sh
4. Prüfen: curl http://<host>:8080/health

Rollback: FERRUM_VERSION auf v0.2.0 setzen, erneut ./install.sh

Details: <Link zur GitHub-Release-Seite>
Support: contact@synapticfour.com
```

---

## Checkliste

- [ ] Fix minimal, kein Scope-Creep
- [ ] `main` gemerged, Tag gepusht
- [ ] Release-Artefakte + SHA256 verifiziert
- [ ] Kunden-Mail / Ticket mit **FERRUM_VERSION**-Hinweis versendet
