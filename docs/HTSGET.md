# Ferrum htsget

JSON tickets are served at `/ga4gh/htsget/v1`. Object identifiers are the same as **DRS** (canonical id, alias, or `drs://host/id` when the host matches `FERRUM_DRS_HOSTNAME`).

**Region slicing is not supported for any organism.** Tickets are whole-object only. Requests that include genomic region parameters (`referenceName`, `start`, `end`, POST `regions`) or `class=header` return **HTTP 400** `InvalidInput`. Service-info documents that limitation. Do not treat a green HelixTest demo job as evidence of region-level htsget.

Service-info `type.version` is the string **`1.3.0`**. That is the string Ferrum emits. The upstream htsget specification document is now **1.3.1**. This page does not change the emitted string to 1.3.1.

## Configuration

| Variable | Purpose |
|----------|---------|
| `FERRUM_PUBLIC_BASE_URL` | Scheme + host (+ optional port) prepended to ticket URLs, e.g. `http://127.0.0.1:8080`. When unset, the ticket URL base is `https://` plus `FERRUM_DRS_HOSTNAME`. That hostname defaults to `localhost`, so the default ticket URL base is `https://localhost`. |
| `FERRUM_DRS_HOSTNAME` | Must match DRS URI host and the host embedded in DRS access URLs. |

Tickets point clients at:

`{ticket URL base}/ga4gh/drs/v1/objects/{id}/stream`

HTTP `Range` on unencrypted `/stream` is applied after a full object read: the handler reads and discards bytes before the requested start, then sends the range. Crypt4GH-encrypted objects reject `Range`.

Use the same `Authorization: Bearer …` (or demo auth) as for DRS when fetching the stream if the object is dataset-controlled.

## Reads vs variants

Classification uses `mime_type` and optional object `name` (e.g. `.bam`, `.vcf`, `.vcf.gz`). Wrong endpoint (e.g. VCF object on `/reads/…`) returns htsget `NotFound`.

## Service info

Per spec, use `/reads/service-info` and `/variants/service-info` (not a single `/service-info` at the v1 root). Descriptions state that region slicing is not implemented.

## Swagger

OpenAPI UI: `/ga4gh/htsget/v1/swagger-ui` (minimal schema).
