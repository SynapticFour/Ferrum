# Metadata Store (optional)

Plan: [IMPLEMENTATION-PLAN-METADATA-STORE.md](METADATA-STORE-ROADMAP.md) · ADR-025 in [DECISIONS.md](../DECISIONS.md)

---

## Kurz

Ferrum speichert ferrum-meta-Submissions schon bei Ingest (`metadata_submissions` + `metadata_ref`). Der **Metadata Store** ist die optionale HTTP-API, um dieselben Dokumente **ohne erneuten Datei-Ingest** zu speichern, zu lesen, zu **versionieren** und an DRS-Objekte zu binden.

**Default: aus.** Ingest-Binding bleibt unverändert.

```toml
[metadata_store]
enabled = true
```

Oder: `FERRUM_METADATA_STORE__ENABLED=true`

### Endpunkte (`/api/v1/metadata`)

| Method | Path | Bedeutung |
|--------|------|-----------|
| `POST` | `/submissions` | Validieren + upsert (Alias aus Dokument) |
| `PUT` | `/submissions/{alias}` | Validieren + upsert (Alias muss matchen) |
| `GET` | `/submissions/{alias}` | Aktuelles Dokument (+ `version`, `content_sha256`, `ETag`) |
| `GET` | `/submissions?profile=&limit=&offset=` | Liste |
| `GET` | `/submissions/{alias}/versions` | Versionshistorie |
| `GET` | `/submissions/{alias}/versions/{n}` | Dokument einer Version |
| `PUT` | `/objects/{object_id}/metadata_ref` | Attach (`{"metadata_ref":"alias"}`) oder Detach (`null`) |

Query `?profile=core|pathogen|h3africa` optional bei Write.

### Optimistic concurrency (M2)

- Header `If-Match: "3"` **oder** Query `?expected_version=3`
- Bei Mismatch → **409** `conflict`
- Antwort setzt `ETag: "<version>"`
- Identischer Inhalt → `unchanged: true` (keine neue Version)

### Auth

Wenn `auth.require_auth=true`: Write → `ferrum:collector`/admin; Read → analyst/collector/admin. Sonst open (wie Ingest).

### Was es **nicht** ist

- Kein EGA/GHGA-Acceptance
- Kein klinischer CDR (Solum)
- Kein Facetten-Search über JSON-Felder (M3)
- Bei `enabled=false`: **501** auf `/api/v1/metadata/*`

DRS liefert weiter nur `metadata_ref` (Pointer). Volltext / Historie: Metadata Store.

---

## English

Optional HTTP API over ferrum-meta submissions already stored at ingest.

Enable with `[metadata_store] enabled = true` or `FERRUM_METADATA_STORE__ENABLED=true`.

**M2:** version history, `If-Match` / `expected_version`, DRS attach/detach via `PUT /objects/{id}/metadata_ref`.

Disabled → HTTP 501. Not archive certification; not clinical SoR.
