# External ONT basecalling (Dorado / Guppy)

Ferrum **does not** run basecalling on Edge nodes. MinKNOW produces raw POD5/FAST5/BLOW5; **Dorado** (GPU) or **Guppy** (legacy) runs on a laptop, eGPU enclosure, or hub GPU pool. Ferrum ingests the **basecalled FASTQ** (or raw archive) via `POST /api/v1/ingest/ont`.

## Recommended field workflow

```text
MinKNOW run → Dorado/Guppy (external) → FASTQ directory
    → ferrum ingest watch --dir …/fastq_pass --meta-bundle collection.yaml
    → ferrum pipeline qc --object-id <drs-id> --fastq sample.fastq
    → (optional hub) ferrum pipeline forward-wes --object-id … --workflow tools/workflows/ont-qc.wdl
```

## Dorado (Oxford Nanopore)

```bash
dorado basecaller sup pod5_out/ --emit-fastq > sample.fastq
ferrum ingest watch --dir ./fastq_pass --gateway http://127.0.0.1:8080
```

Configure Dorado model cache on USB SSD before going offline. Document model version in ferrum-meta `study` notes.

## Guppy (legacy)

```bash
guppy_basecaller -i pod5_out -s fastq_out --flowcell FLO-MIN114 --kit SQK-LSK114
```

## QC metrics callback

After basecalling, attach QC via:

```bash
ferrum pipeline qc --object-id <drs-id> --fastq sample.fastq --allow-stub
```

Or POST directly:

```http
POST /api/v1/ingest/ont-metrics
Content-Type: application/json

{
  "drs_object_id": "<drs-id>",
  "quality_metrics": {
    "mean_qscore": 12.5,
    "read_count": 10000,
    "n50": 15000,
    "read_length_histogram": []
  }
}
```

WES template [`tools/workflows/ont-qc.wdl`](../tools/workflows/ont-qc.wdl) calls the same endpoint after NanoStat/NanoPlot.

## Config

```toml
[pipeline]
nanostat_bin = "/usr/local/bin/NanoStat"
allow_qc_stub = false   # true for CI/demo without NanoStat installed
```

## Related

- [INGEST-LAB-KIT.md](INGEST-LAB-KIT.md)
- [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md)
- [FIELD-BEACON-INDEX.md](FIELD-BEACON-INDEX.md)
