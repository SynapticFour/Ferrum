# Ferrum htsget

GA4GH **htsget** 1.3.0-style JSON tickets are served at `/ga4gh/htsget/v1`. Object identifiers are the same as **DRS** (canonical id, alias, or `drs://host/id` when the host matches `FERRUM_DRS_HOSTNAME`).

## Configuration

| Variable | Purpose |
|----------|---------|
| `FERRUM_PUBLIC_BASE_URL` | Scheme + host (+ optional port) prepended to ticket URLs, e.g. `http://127.0.0.1:8080`. Default: `https://{FERRUM_DRS_HOSTNAME}`. |
| `FERRUM_DRS_HOSTNAME` | Must match DRS URI host and the host embedded in DRS access URLs. |

Tickets point clients at:

`{FERRUM_PUBLIC_BASE_URL}/ga4gh/drs/v1/objects/{id}/stream`

Use the same `Authorization: Bearer …` (or demo auth) as for DRS when fetching the stream if the object is dataset-controlled.

## Reads vs variants

Classification uses `mime_type` and optional object `name` (e.g. `.bam`, `.vcf`, `.vcf.gz`). Wrong endpoint (e.g. VCF object on `/reads/…`) returns htsget `NotFound`.

## Service info

Per spec, use `/reads/service-info` and `/variants/service-info` (not a single `/service-info` at the v1 root).

## Swagger

OpenAPI UI: `/ga4gh/htsget/v1/swagger-ui` (minimal schema).
