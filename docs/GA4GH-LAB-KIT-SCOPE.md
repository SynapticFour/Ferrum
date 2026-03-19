# Scope: GA4GH Compliance Starter Kit vs. Ferrum

**Zweck dieses Dokuments:** Festlegen, **was im Ferrum-Monorepo** gebaut und gewartet wird und **was** besser als **separates Kit-Repo** (Compose-/Helm-Profile, Checklisten, Audit-Artefakte) lebt – damit Produktgrenze, Roadmap und Ownership klar bleiben.

**Zielpersona:** Kleines Lab oder CRO, das einem Konsortium beitreten will und **nachweisbare GA4GH-Nähe** (laufende Services + Tests) braucht – nicht zwingend eine vollständige Forschungsplattform.

---

## 1. Repo-Grenze (Entscheidungsregel)

| Kriterium | **Ferrum (dieses Repo)** | **Kit-Repo** (z. B. `ga4gh-lab-kit` / `compliance-starter`) |
|-----------|---------------------------|---------------------------------------------------------------|
| **Inhalt** | Laufende Implementierung von APIs, Persistenz, Gateway, Rust-Crates, CI für Code | Orchestrierung **über** Ferrum: Compose-Overlays, Helm values, **Audit-Checklisten**, Runbooks, „So führst du Conformance aus“, ggf. Wrapper-Skripte |
| **Nutzer** | Entwickler:innen, die Services erweitern | Lab-IT / Bioinformatik: „deploy + verifizieren“ |
| **Release** | Versionierte Gateway-/Service-Images | Versioniertes „Kit“-Release (welche Ferrum-Version + welche optionalen Services) |

**Regel:** Alles, was **HTTP-API-Verhalten, DB-Schema oder Sicherheitslogik** braucht → **Ferrum**. Alles, was **nur zusammenstellt, dokumentiert und testet** → **Kit** (kann Ferrum-Images pinned referenzieren).

---

## 2. Komponenten-Matrix

| Komponente | Ferrum heute | **Phase 1** (MVP, ~3–4 Mon.) | **Phase 2** | Kit-Repo (beide Phasen) |
|------------|--------------|------------------------------|-------------|-------------------------|
| **DRS** | `ferrum-drs`, Gateway | Nutzung + ggf. kleine Lücken für Kit-Szenarien | Presigned/Access-Patterns für Lab-Workflows dokumentieren | Compose-Profil „nur Daten“; Checkliste „DRS-Objekt + Access“ |
| **Beacon v2** | `ferrum-beacon`, Gateway | Feature-Flag / Seed-Daten für Kit-Demo; ggf. `supports_beacon_v2` in Tests abstimmen | Feinabstimmung mit Consortium-Anforderungen | Profil „Beacon an“; Integrationsguide |
| **htsget** | **`ferrum-htsget`** + Gateway (`/ga4gh/htsget/v1`), Tickets → DRS `/stream`; HelixTest in CI | Feintuning (Intervalle, weitere Fehlerfälle), optional Sidecar nur wenn nötig | Stabilisierung, Auth an Passport/AAI, Performance | Kit: Profil „htsget an“, Checkliste Ticket → Stream |
| **WES / TES / TRS** | Voll im Stack | **Optional im Kit-MVP** aus (reduziert Supportfläche) oder als „erweitertes Profil“ | Für Konsortien mit Workflow-Nachweis | Profil `full-stack` vs. `compliance-minimal` |
| **Phenopackets** | Nicht vorhanden | — | **Ingestion-API oder Batch-Importer** (neues Modul/Crate) + minimales Speichermodell; *kein* volles Pheno-Store-Produkt im MVP | Import-Runbook; Mapping-Tabelle Lab-Felder → Phenopackets |
| **Auth (ELIXIR AAI / OIDC)** | Keycloak/JWT/Passports vorhanden | An **einem** Referenzpfad festnageln (z. B. ELIXIR AAI + Keycloak-Realm-Template) | HPC/K8s-Varianten dokumentieren | Schritt-für-Schritt-Guide; **keine** Secrets im Repo |
| **Conformance / Tests** | HelixTest-CI, siehe [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) | Kit referenziert dieselben Tests + **menschenlesbare Checkliste** („welcher Schritt beweist was“) | Ergänzung um htsget/Phenopackets, sobald Spec-Tests existieren | `docs/audit-checklist.md`, Report-Vorlage für Paid Audit |

---

## 3. Phase 1 – MVP (empfohlener Scope)

- **Enthalten:** Referenz-Deployment (Compose, optional ein Helm-Chart) mit **Beacon v2 + htsget + DRS**; ein durchgängiger **„grüner“ Conformance-/Smoke-Pfad** (HelixTest in CI, siehe [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md)).
- **Ferrum-Arbeit:** htsget ist **im Monorepo**; Schwerpunkt **Beacon** für Kit-Szenario **konfigurierbar/seedbar**; Gateway-Routen und Dokumentation mit Kit-Checklisten abstimmen.
- **Kit-Arbeit:** Profil `compliance-minimal`; Dokumentation **Bare-Metal/HPC (SLURM nur als Client-Story, nicht vollständiger Scheduler-Betrieb im MVP), K8s, AAI** – jeweils **eine** happy-path-Anleitung statt drei produktionsreife Varianten.
- **Explizit nicht im MVP:** Vollständige EGA/Genomics-England-Integration (das ist **Paid Consulting**); alle vier Umgebungen produktionshart.

---

## 4. Phase 2 – Ausbau

- **Phenopackets-Ingestion** in Ferrum (schmales, auditierbares Modul).
- Kit: zweites Profil „Genomics + Phenotypes“; erweiterte Checklisten.
- Optional: WES/TES im Kit als „Extended Profile“ für Labs mit Workflow-Nachweis.

---

## 5. Nicht-Ziele (Scope begrenzen)

- Kein Ersatz für nationale Dateninfrastrukturen (EGA etc.) im Open-Core.
- Keine Garantie „offizielle GA4GH-Zertifizierung“ – nur **technischer Nachweis** über definierte Tests + Checklisten.
- Kit-Repo **hostet keine** kundenspezifischen Audit-Reports (die leben beim Dienstleister / beim Kunden).

---

## 6. Paid Services (außerhalb der Repos)

**Compliance-Audit** (Review Deployment, Testläufe, schriftlicher Report) und **Integrationsberatung** (ELIXIR, 1+MG, …) sind **Dienstleistungen** – nicht Teil des Open-Source-Scopes, aber das Kit liefert die **reproduzierbare Basis**, auf der Audits aufsetzen.

---

## 7. Nächste konkrete Schritte

1. **htsget:** In Ferrum als Crate + Routing umgesetzt; Kit-Repo kann darauf referenzieren (keine separate „Entscheidung“ mehr nötig, außer bei Sonderfall Sidecar).
2. **Kit-Repo anlegen** (oder `deploy/lab-kit/` im Monorepo **nur** wenn die Marke „Ferrum“ bewusst die Kit-Dachmarke wird).
3. **MVP-Compose-Profil** definieren: Services, Ports, env-Variablen, Healthchecks – pinned auf Ferrum-Image-Tags.
4. **Audit-Checkliste v0.1** (Markdown): eine Zeile pro Nachweis, verlinkt auf Testkommando oder API-Call.
5. **Roadmap-Review** nach 6–8 Wochen: Scope einhalten oder Phase 2 strecken.

---

*[← Dokumentationsindex](README.md)*
