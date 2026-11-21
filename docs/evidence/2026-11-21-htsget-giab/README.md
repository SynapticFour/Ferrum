# htsget region tickets on GIAB HG002 data (2026-11-21)

Recorded report. Ferrum **v0.3.12** release binary, htsget region and header tickets checked against public Genome in a Bottle (GIAB) HG002 data on GRCh38.

## Result

36 of 36 cases passed. In every case the ticket, fetched and concatenated, is a file that samtools/bcftools read without a warning; it contains every record of the requested region(s) at least as often as the original file does, and no record more often than the whole original file has it. 7 of the 36 regions have no records in the original slice (they fall into gaps of the data); for those the check is that the ticket is valid and adds nothing.

Not covered by this run: CRAM (covered by `crates/ferrum-htsget/tests/regions.rs` with htslib-written test files only), Crypt4GH tickets, and the official GA4GH htsget compliance suite.

## What was run

- **Script:** [`scripts/giab-htsget-check.sh`](../../../scripts/giab-htsget-check.sh) at the commit that adds this report.
- **Binary:** `ferrum-gateway-aarch64-apple-darwin.tar.gz` from the GitHub release v0.3.12, SHA-256 `711b6f86597189f6d6d77704773757fed60d8127ef0f4686ae01089eb07625c3` (checked against the release `SHA256SUMS.txt`), `edge` features, run on macOS arm64 with a local SQLite store and `require_auth = false` on 127.0.0.1.
- **Repeat:** the same 36 cases on the same data with `ferrum-gateway-x86_64-unknown-linux-musl.tar.gz` from the same release (SHA-256 `82e0ec43b9110b3f05105b70e5ecb7252ccd9bf53c5fb1444a3bba9b5bf29553`) on Linux x86_64 gave the same result, with byte-identical ticket sizes.
- **Tools:** samtools, bcftools, bgzip, tabix (htslib) for the slices and the comparison.

## Data

Slices fetched with samtools/bcftools through the index (only the slice is transferred) from `https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/`:

| File | Source | Slice | Size |
|------|--------|-------|-----:|
| `illumina.bam` + BAI | HG002 NIST Illumina 2x250, novoalign, `HG002.GRCh38.2x250.bam` | chr20:30,000,000-31,000,000 | 41,884,048 |
| `ont.bam` + BAI | HG002 ultralong Oxford Nanopore, `HG002_GRCh38_ONT-UL_GIAB_20200122.phased.bam` | chr20:30,000,000-32,000,000 | 217,731,476 |
| `benchmark.vcf.gz` + TBI | NIST v4.2.1 benchmark, `HG002_GRCh38_1_22_v4.2.1_benchmark.vcf.gz` | chr20, chr21 | 5,310,581 |
| `benchmark.bcf` + CSI | the same records, written by bcftools | chr20, chr21 | 5,992,095 |

The ONT file has reads much longer than one BGZF block, so records span blocks; this is the case where block-aligned byte ranges alone do not form a valid file.

## Method

1. Upload the eight files through `POST /ga4gh/drs/v1/ingest/file`; the index objects are found by name (`.bai`, `.tbi`, `.csi`).
2. For each case, `POST /ga4gh/htsget/v1/{reads|variants}/{id}` with one or more regions (0-based, end exclusive), fetch every ticket URL (`Range` requests and `data:` URLs) and concatenate.
3. Expected records: `samtools view` / `bcftools view -H` of the original file for each region (1-based closed), as whole lines, with the original multiplicity; for several regions in one request the maximum per record (overlapping regions must not duplicate records).
4. Pass when the ticket parses without a warning, nothing expected is missing, and no record appears more often than in the whole original file. A ticket may contain records outside the region (the index works in blocks); clients filter.
5. `class=header` for one BAM and one BCF: the ticket must parse and hold no records.

Regions were drawn with a fixed random seed (20) inside the slices.

## Table (macOS arm64 run)

`expected` = records in the region(s) of the original; `received` = records in the ticket (block granularity); `missing` / `too often` as in step 4.

| File | Region(s) | expected | received | missing | too often | ticket bytes | file bytes | result |
|------|-----------|---------:|---------:|--------:|----------:|-------------:|-----------:|--------|
| illumina.bam | chr20:30000000-30001000 | 55 | 1053 | 0 | 0 | 198868 | 41884048 | ok |
| illumina.bam | chr20:30995000-31000000 | 1532 | 5154 | 0 | 0 | 768641 | 41884048 | ok |
| illumina.bam | chr20:30949632-30951632 | 580 | 5106 | 0 | 0 | 767291 | 41884048 | ok |
| illumina.bam | chr20:30272448-30272548 | 70 | 4504 | 0 | 0 | 674141 | 41884048 | ok |
| illumina.bam | chr20:30914494-30934494 | 10980 | 22691 | 0 | 0 | 3488292 | 41884048 | ok |
| illumina.bam | chr20:30600771-30602771 | 599 | 5295 | 0 | 0 | 825604 | 41884048 | ok |
| illumina.bam | chr20:30028290-30078290 | 452 | 1748 | 0 | 0 | 319318 | 41884048 | ok |
| illumina.bam | chr20:30426498-30426598 | 0 | 1284 | 0 | 0 | 225827 | 41884048 | ok (region empty in the original) |
| illumina.bam | 3 regions (POST) | 11630 | 32023 | 0 | 0 | 4835677 | 41884048 | ok |
| ont.bam | chr20:30000000-30001000 | 260 | 776 | 0 | 0 | 20205567 | 217731476 | ok |
| ont.bam | chr20:31995000-32000000 | 72 | 197 | 0 | 0 | 10038564 | 217731476 | ok |
| ont.bam | chr20:30216526-30218526 | 206 | 922 | 0 | 0 | 26629777 | 217731476 | ok |
| ont.bam | chr20:30669283-30719283 | 205 | 7146 | 0 | 0 | 85487087 | 217731476 | ok |
| ont.bam | chr20:31217743-31267743 | 945 | 1625 | 0 | 0 | 20362582 | 217731476 | ok |
| ont.bam | chr20:30863988-30865988 | 354 | 7674 | 0 | 0 | 82323048 | 217731476 | ok |
| ont.bam | chr20:30419044-30439044 | 453 | 7321 | 0 | 0 | 88711074 | 217731476 | ok |
| ont.bam | chr20:31311503-31331503 | 139 | 1305 | 0 | 0 | 21662731 | 217731476 | ok |
| ont.bam | 3 regions (POST) | 1356 | 9255 | 0 | 0 | 115318281 | 217731476 | ok |
| benchmark.vcf.gz | chr20:30000000-31000000 | 0 | 0 | 0 | 0 | 3129 | 5310581 | ok (region empty in the original) |
| benchmark.vcf.gz | chr21:15000000-15200000 | 186 | 194 | 0 | 0 | 11076 | 5310581 | ok |
| benchmark.vcf.gz | chr20:0-70000 | 0 | 0 | 0 | 0 | 3129 | 5310581 | ok (region empty in the original) |
| benchmark.vcf.gz | chr20:23061447-23161447 | 205 | 228 | 0 | 0 | 12054 | 5310581 | ok |
| benchmark.vcf.gz | chr20:7222621-7322621 | 173 | 179 | 0 | 0 | 10145 | 5310581 | ok |
| benchmark.vcf.gz | chr20:28174593-28175593 | 0 | 0 | 0 | 0 | 3129 | 5310581 | ok (region empty in the original) |
| benchmark.vcf.gz | chr20:14847464-14848464 | 2 | 29 | 0 | 0 | 4968 | 5310581 | ok |
| benchmark.vcf.gz | 2 regions (POST) | 186 | 194 | 0 | 0 | 11076 | 5310581 | ok |
| benchmark.bcf | chr20:30000000-31000000 | 0 | 0 | 0 | 0 | 3766 | 5992095 | ok (region empty in the original) |
| benchmark.bcf | chr21:15000000-15200000 | 186 | 194 | 0 | 0 | 12528 | 5992095 | ok |
| benchmark.bcf | chr20:0-70000 | 0 | 0 | 0 | 0 | 3766 | 5992095 | ok (region empty in the original) |
| benchmark.bcf | chr20:3355330-3356330 | 2 | 20 | 0 | 0 | 5358 | 5992095 | ok |
| benchmark.bcf | chr20:7343463-7344463 | 0 | 2 | 0 | 0 | 4215 | 5992095 | ok (region empty in the original) |
| benchmark.bcf | chr20:13773336-13873336 | 106 | 121 | 0 | 0 | 9426 | 5992095 | ok |
| benchmark.bcf | chr20:47535851-47635851 | 100 | 112 | 0 | 0 | 9758 | 5992095 | ok |
| benchmark.bcf | 2 regions (POST) | 186 | 194 | 0 | 0 | 12528 | 5992095 | ok |
| illumina.bam | class=header | 0 | 0 | | | 24447 | | ok |
| benchmark.bcf | class=header | 0 | 0 | | | 3766 | | ok |

## Observations

- **Ticket size for ultralong reads.** A 2 kb to 50 kb region of the ONT slice returns 10 to 89 MB (the BAI bins and the reads that overlap them are long). The tickets are correct; for such data they are not small. A client still filters by region.
- **Duplicate keys in the source.** The ONT file holds records that share read name, flag, reference and position (570 such keys in the slice). A first version of the check compared only those four fields and reported them as duplicates; the check now compares whole records against the original multiplicity.
- **Empty regions.** The benchmark VCF has no calls in chr20:30-31 Mb (near the centromere) and at the chromosome start; the seed placed several regions there.
