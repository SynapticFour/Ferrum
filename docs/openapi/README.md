# Ferrum OpenAPI dump (implementation map)

File: [`ferrum.openapi.json`](ferrum.openapi.json)

**Source of truth for GA4GH products is the published OpenAPI of that standard**, not this file.

| Product | Spec to implement / validate against |
|---------|--------------------------------------|
| DRS | [data-repository-service-schemas](https://ga4gh.github.io/data-repository-service-schemas/) |
| WES | [workflow-execution-service-schemas](https://ga4gh.github.io/workflow-execution-service-schemas/) |
| TES | [task-execution-service-schemas](https://ga4gh.github.io/task-execution-service-schemas/) |
| TRS | [tool-registry-service-schemas](https://ga4gh.github.io/tool-registry-service-schemas/) |
| Beacon v2 | Beacon v2 framework schemas |
| htsget | [hts-specs `htsget-openapi.yaml`](https://github.com/samtools/hts-specs) |
| Passports / visas | [ga4gh-passport-v1](https://github.com/ga4gh-duri/ga4gh-passport-v1) (JWT claims, not this dump) |

HelixTest already vendors those OpenAPI documents (WES, TES, TRS, htsget) and validates responses against them. Ferrum’s README / [GA4GH.md](../GA4GH.md) link the same URLs.

## What this file is

A **committed dump of what this Ferrum gateway process exposes**, generated from the same **utoipa** types as runtime `/openapi.json`. Use it to see:

- Gateway-absolute paths (`/ga4gh/drs/v1/service-info`, not `/service-info`)
- **Ferrum-only** additions (DRS `/stream`, ingest, Passports HTTP, Cohorts, Crypt4GH HTTP, admin routes we export)
- Drift: `make prove` / `make openapi-check` fail if someone changed utoipa and forgot to commit the JSON

Pin at a **Ferrum git tag** when you need “what did this binary claim to serve”, not when you need the GA4GH standard.

## What this is not

- Not a replacement for the published GA4GH OpenAPI
- Not official **GA4GH certification**
- Not a complete inventory of every internal route. If a path is missing here, it is not a supported Ferrum integration surface yet
- Not an auth policy. `make up` is auth-off. `make eval` is HS256. Passports/AAI are `make up-pilot-local`

## Regenerate

```bash
make openapi        # write the file
make prove          # cargo test, including dump-vs-utoipa drift
```
