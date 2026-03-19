# Workflow execution guide

This guide covers submitting workflows (Nextflow, CWL, WDL, Snakemake) via the WES API and Ferrum UI, using DRS objects as inputs, HPC execution (SLURM/LSF), and live log streaming.

---

## Nextflow

### Submit via WES API

```bash
curl -X POST "https://ferrum.example.com/ga4gh/wes/v1/runs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_params": {},
    "workflow_type": "NFL",
    "workflow_type_version": "DSL2",
    "workflow_url": "https://github.com/org/repo/main.nf",
    "workflow_engine_params": {
      "project": "my-slurm-project",
      "queue": "normal"
    }
  }'
```

### Submit via Ferrum UI

Use the **Workflows** tab: paste workflow URL (or upload), set params, optional `workflow_engine_params`, and submit. Run status and logs are shown in the UI.

### DRS inputs

Use `drs://` URIs in `workflow_params` or in the workflow. Ferrum WES resolves them to access URLs (or local paths when using a shared filesystem):

```json
{
  "workflow_params": {
    "input_bam": "drs://ferrum.example.com/ga4gh/drs/v1/objects/sample01"
  }
}
```

### Engine params (example)

| Param | Description |
|-------|-------------|
| `project` | SLURM account / project |
| `queue` | Partition name |
| `max_retries` | Number of retries |

### Example workflow

```groovy
// main.nf
nextflow.enable.dsl = 2
process run_bam {
  input: path bam
  output: path "out.txt"
  script: "samtools flagstat $bam > out.txt"
}
workflow {
  run_bam(Channel.fromPath(params.input_bam))
}
```

---

## CWL (Common Workflow Language)

### Submit via WES API

```bash
curl -X POST "https://ferrum.example.com/ga4gh/wes/v1/runs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_params": { "input_file": {"class": "File", "location": "drs://host/objects/abc"} },
    "workflow_type": "CWL",
    "workflow_type_version": "v1.2",
    "workflow_url": "https://example.com/workflow.cwl",
    "workflow_engine_params": {}
  }'
```

### DRS integration

Use `location: "drs://host/ga4gh/drs/v1/objects/{id}"` in workflow params. WES resolves to a staged file or URL for the engine (e.g. cwltool).

### Engine

Ferrum uses **cwltool** (or configured CWL runner). Set `workflow_engine_params` as needed for your runner.

---

## WDL (Cromwell)

### Submit via WES API

```bash
curl -X POST "https://ferrum.example.com/ga4gh/wes/v1/runs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_params": {},
    "workflow_type": "WDL",
    "workflow_type_version": "1.1",
    "workflow_url": "https://example.com/pipeline.wdl",
    "workflow_engine_params": { "project": "myproject" }
  }'
```

### DRS inputs

Reference DRS objects in the WDL inputs JSON, e.g. `"input_bam": "drs://host/ga4gh/drs/v1/objects/xyz"`. WES resolves before invoking Cromwell.

---

## Snakemake

### Submit via WES API

```bash
curl -X POST "https://ferrum.example.com/ga4gh/wes/v1/runs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_type": "SMK",
    "workflow_type_version": "8",
    "workflow_url": "https://example.com/Snakefile",
    "workflow_params": { "input": "drs://host/objects/abc" },
    "workflow_engine_params": { "cores": 4 }
  }'
```

### Engine params

| Param | Description |
|-------|-------------|
| `cores` | Max cores per run |
| `jobs` | Max parallel jobs |
| SLURM/LSF | Same as Nextflow (project, queue, etc.) when using HPC backend |

---

## HPC execution

Ferrum WES/TES submit jobs to **SLURM** or **LSF** via `workflow_engine_params` and executor configuration in Ferrum config.

- **Resource specification:** Use engine params such as `queue`, `project`, `memory`, `cpus`, `walltime` (names may vary by executor).
- **Monitoring:** Use WES `GET /runs/{id}` and `/runs/{id}/logs` or the Ferrum UI. For SLURM, jobs appear in `squeue`; Ferrum tracks run_id ↔ job id internally.

---

## Live log streaming

Stream run logs over SSE:

```bash
curl -N -H "Authorization: Bearer $TOKEN" \
  "https://ferrum.example.com/ga4gh/wes/v1/runs/$RUN_ID/logs/stream"
```

Events are emitted as the workflow produces stdout/stderr. The Ferrum UI uses this endpoint for the live log view.

---

## DRS integration summary

| Feature | Description |
|---------|-------------|
| **Inputs** | Use `drs://<ferrum-host>/ga4gh/drs/v1/objects/<id>` in workflow params or workflow definitions. |
| **Resolution** | WES resolves to access URL (or staged path); auth (Passport) is passed so DRS returns authorized URLs or streams. |
| **Outputs** | Configure auto-ingest of workflow outputs back into DRS (implementation-specific); otherwise register outputs manually via DRS ingest API. |

---

*[← Documentation index](README.md)*
