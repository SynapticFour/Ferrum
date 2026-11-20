# Ferrum htsget

JSON tickets are served at `/ga4gh/htsget/v1`. Object identifiers are the same as **DRS** (canonical id, alias, or `drs://host/id` when the host matches `FERRUM_DRS_HOSTNAME`).

Service-info `type.version` is the string **`1.3.0`**. That is the string Ferrum emits. The upstream htsget specification document is now **1.3.1**. This page does not change the emitted string to 1.3.1.

## What a ticket contains

| Request | Ticket |
|---------|--------|
| No `referenceName` / `regions`, no `class` | One URL: the whole object, `{base}/ga4gh/drs/v1/objects/{id}/stream`. No index needed. |
| `referenceName` (+ optional `start` / `end`), or POST `regions` | Header bytes, the bytes that hold the records of the region(s), and the end-of-file marker. Needs an index (below). |
| `class=header` | Header bytes and the end-of-file marker. Needs an index. |

`start` is 0-based inclusive and `end` exclusive, as in the htsget spec. A ticket can contain records just outside the region (the index works in blocks or containers); clients filter. Overlapping POST regions do not duplicate records. `fields`, `tags` and `notags` are accepted and not applied; service-info says `fieldsParameterEffective: false`.

Region tickets are computed by [htsget-rs](https://github.com/umccr/htsget-rs) (`htsget-search` / `htsget-storage`, MIT) over Ferrum's object storage. URLs are either the DRS stream with an HTTP `Range` header, or `data:` URLs with a few bytes inline (end-of-file markers, cut blocks). Fetch them in order and concatenate.

For plain BAM, VCF.gz and BCF, Ferrum builds the ticket from the same index with noodles: the header ends at the first record's position in the index, and the first and last BGZF block of each index chunk are cut at the record boundary the index records; a cut block is served as a `data:` URL. Block-aligned ranges alone are not a valid file when records span BGZF blocks or the header shares its last block with records. htslib writes such files: every BCF, and BAM or VCF records longer than one block (ultra-long reads, VCFs with many samples).

## Index objects

A region or header request on an object without an index returns **404** `NotFound` with a hint. Ferrum finds the index:

1. from the data object's metadata key `htsget.index` (value: DRS id of the index object), otherwise
2. as the object named `<data object name><suffix>` in the same dataset:

| Format | Index suffix |
|--------|--------------|
| BAM | `.bai` |
| CRAM | `.crai` |
| VCF.gz | `.tbi` |
| BCF | `.csi` |

htsget-rs reads BAI for BAM and TBI for VCF.gz, so a CSI index is not used for those two formats. Write files and indexes with htslib (samtools, bcftools, bgzip, tabix) or tools that write the same layout. Data and index must be in this node's object storage (`local`, `s3` or `minio` backend); URL-registered objects get whole-object tickets only.

## Crypt4GH objects

For an object stored encrypted (`is_encrypted`), a region or header request needs the requester's X25519 public key in the header `X-Crypt4GH-Public-Key` (or `Htsget-Context-Public-Key`, as htsget-rs names it): base64 of the raw 32-byte key or of a Crypt4GH public key file. Without it the request returns **400**.

The ticket starts with a Crypt4GH header re-encrypted for that key, with a data edit list, followed by ranges of the stored ciphertext (`/stream?raw=true`). The client decrypts the concatenation with its own private key. The node key never leaves the node; it is read from `encryption.crypt4gh_key_dir` / `{crypt4gh_master_key_id}.sec`, as for DRS. The index object stays plain.

Limits for Crypt4GH region tickets:

- One region per request (POST with several `regions` returns 400). Send one request per region.
- Records must not span BGZF blocks, because ciphertext blocks are not cut. htslib BAM and bgzip VCF.gz meet this unless a single record is longer than one block.
- BCF: region requests return **400** `UnsupportedFormat`; header and whole-object tickets work.

## Configuration

| Variable | Purpose |
|----------|---------|
| `FERRUM_PUBLIC_BASE_URL` | Scheme + host (+ optional port) prepended to ticket URLs, e.g. `http://127.0.0.1:8080`. When unset, the ticket URL base is `https://` plus `FERRUM_DRS_HOSTNAME`. That hostname defaults to `localhost`, so the default ticket URL base is `https://localhost`. |
| `FERRUM_DRS_HOSTNAME` | Must match DRS URI host and the host embedded in DRS access URLs. |

For plain objects, `Range` on `/stream` reads only the requested bytes from storage (local seek, S3 `Range`, OpenDAL range read). On Crypt4GH objects `/stream` decrypts for the caller and rejects `Range`; `/stream?raw=true` returns the stored ciphertext and accepts `Range`, which is what Crypt4GH tickets use.

Use the same `Authorization: Bearer …` (or demo auth) as for DRS when fetching the stream if the object is dataset-controlled. Ticket requests run the same dataset checks as DRS.

## Reads vs variants

Classification uses `mime_type` and optional object `name` (e.g. `.bam`, `.cram`, `.vcf`, `.vcf.gz`, `.bcf`). Wrong endpoint (e.g. VCF object on `/reads/…`) returns htsget `NotFound`.

## Service info

Per spec, use `/reads/service-info` and `/variants/service-info` (not a single `/service-info` at the v1 root). The descriptions state the index and Crypt4GH requirements above.

## Tests

`crates/ferrum-htsget/tests/regions.rs` stores BAM/BAI, VCF.gz/TBI, CRAM/CRAI and BCF/CSI files as DRS objects, requests tickets, fetches every URL through the DRS stream route and parses the result. Expected records come from the original file and its index (noodles), not from htsget-rs. The CRAM and BCF fixtures are written by htslib (`tests/data/make_fixtures.py`). The tests cover records that span BGZF blocks, multi-region POST, `class=header`, missing index, unknown reference, and Crypt4GH tickets that only the requester's key decrypts.

## Swagger

OpenAPI UI: `/ga4gh/htsget/v1/swagger-ui` (minimal schema).
