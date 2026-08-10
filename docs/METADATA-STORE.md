# Metadata Store (optional)

[🇬🇧 English](#english) · Plan: [IMPLEMENTATION-PLAN-METADATA-STORE.md](internal/IMPLEMENTATION-PLAN-METADATA-STORE.md) · ADR-025 in [DECISIONS.md](../DECISIONS.md)

---

## Kurz

Ferrum speichert ferrum-meta-Submissions schon bei Ingest (`metadata_submissions` + `metadata_ref`). Der **Metadata Store** ist die optionale HTTP-API, um dieselben Dokumente **ohne erneuten Datei-Ingest** zu speichern, zu lesen und zu listen.

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
| `GET` | `/submissions/{alias}` | Volles Dokument |
| `GET` | `/submissions?profile=&limit=&offset=` | Liste |

Query `?profile=core|pathogen|h3africa` optional bei Write.

### Auth

Wenn `auth.require_auth=true`: Write → `ferrum:collector`/admin; Read → analyst/collector/admin. Sonst open (wie Ingest).

### Was es **nicht** ist

- Kein EGA/GHGA-Acceptance
- Kein klinischer CDR (Solum)
- Kein Versioning / Facetten-Search (kommt in M2/M3)
- Bei `enabled=false`: **501** auf `/api/v1/metadata/*`

DRS liefert weiter nur `metadata_ref` (Pointer). Volltext: Metadata Store GET.

---

<a name="english"></a>

## English

Ferrum already stores ferrum-meta bundles at ingest. The **Metadata Store** is the optional HTTP API to **put / get / list** those documents without re-running file ingest.

Enable with `[metadata_store] enabled = true` or `FERRUM_METADATA_STORE__ENABLED=true`.

See table above for routes. Disabled → HTTP 501. Not archive certification; not clinical SoR.
