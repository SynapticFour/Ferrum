# Field Beacon indexing (VCF → SQLite)

Edge nodes can index **small VCF** files into the local Beacon SQLite table for immediate `exists` queries without publishing to a hub ADS dataset.

## Automatic indexing

When `[pipeline] auto_index_beacon = true` (default on Edge), ingest of `.vcf` / `.vcf.gz` objects triggers a background job:

1. Read object bytes from local storage
2. Parse SNV rows (max **10 000** variants, max **50 MB** file)
3. Insert into `beacon_variants` under dataset id `[pipeline] default_beacon_dataset` (default: `field-edge`)

Status is stored in `drs_object_metadata` key `vcf_index_status` (`pending`, `running`, `completed:N`, `failed:…`).

## Manual CLI

```bash
ferrum pipeline index-beacon --object-id <drs-id> [--dataset field-edge]
```

## Limits (documented for operators)

| Limit | Value | Rationale |
|-------|-------|-----------|
| Max variants indexed | 10 000 | Pi SQLite write budget |
| Max VCF size | 50 MB | Memory on Edge |
| Variant types | SNV only | Phase 5 scope |
| Multiallelic | First ALT only | Simplified parser |

Large cohort VCFs: index on **hub** after `ferrum sync push`, or use publish UI with `index_variants`.

## Query

```http
GET /ga4gh/beacon/v2/g_variants?referenceName=chr1&start=100&referenceBases=A&alternateBases=G
```

## Related

- [FEDERATION.md](FEDERATION.md)
- `crates/ferrum-beacon/src/vcf_index.rs`
