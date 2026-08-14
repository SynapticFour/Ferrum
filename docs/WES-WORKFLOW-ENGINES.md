# WES workflow engines (WDL, Nextflow, CWL, Snakemake)

Ferrum **WES** routes runs to an executor from **`workflow_type`** (and optional **`workflow_engine_params`**) on the run request. This matches common **GA4GH WES** expectations: clients submit **`workflow_type` + `workflow_url`** (and attachments).

---

## Supported `workflow_type` values (Ferrum)

| `workflow_type` (case-insensitive) | Direct executor (no TES) | When **`FERRUM_WES_TES_URL`** / TES is configured |
|-----------------------------------|---------------------------|---------------------------------------------------|
| **`wdl`** | Cromwell-style command (see `CromwellExecutor`) | TES: **`broadinstitute/cromwell:93-0232cbd`** + bash launcher. Optional **`FERRUM_WES_TES_WDL_BASH_LAUNCH`** adds `inputs.json` + workdir binds (see [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md)). |
| **`nextflow`** or **`nxf`** | `NextflowExecutor` | TES: **`nextflow/nextflow:24.10.3`** (bash: curl + `nextflow run`). Optional **`FERRUM_WES_TES_NEXTFLOW_FILE_LAUNCH`** adds `params.json` + workdir binds. |
| **`cwl`** | `cwltool` | TES: **`quay.io/commonwl/cwltool:3.2.20260413085819`** (bash launcher; TRS URLs via `FERRUM_WES_GATEWAY_INTERNAL_URL`) |
| **`snakemake`** | `snakemake` | TES: **`snakemake/snakemake:v7.32.4`** |

**Apple Silicon:** set **`FERRUM_TES_DOCKER_PLATFORM=linux/amd64`** (`make up-tes` does this by default) — **`nextflow/nextflow`** is amd64-only.

**Source:** `crates/ferrum-wes/src/run_manager.rs` (`executor_for_type`), `crates/ferrum-wes/src/executors/tes.rs` (`build_tes_task_request`, `legacy_executor_body`).

**No fork required for Nextflow** — submit WES with `workflow_type: "Nextflow"` (or `nextflow` / `nxf`) and a **`workflow_url`** pointing at your script (e.g. TRS URL, `https://`, or `file:` where your deployment allows it). TES must be reachable and the **task image** must contain a working **Nextflow** install (default public image above).

---

## `workflow_engine_params` (examples)

| Key | Effect |
|-----|--------|
| **`ferrum_backend`** / **`ferrum-backend`** | Value **`slurm`** forces **Slurm** when TES is **not** configured. Value **`lsf`** returns a validation error (**LSF is not implemented**). |

For **Docker / Podman TES** (long runs, scratch space, nested engines), see **[TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md)** — **`executors[].entrypoint`**, host bind mounts, WES → TES volume strategy.

---

## Roadmap / gaps (for benchmark repos)

| Topic | Status |
|-------|--------|
| **Nextflow + DRS URI inputs** | WES resolves **`drs://`** for workflow inputs when using the DRS client path; align **TES** mounts with engine expectations (see TES doc). |
| **Custom Nextflow images / JVM flags** | Default TES path uses **public** images; per-run **`container_image`** / arbitrary TES JSON is still a product gap — use custom TES clients or extend Ferrum when needed. |
| **WDL / nested Docker** | Opt-in **`FERRUM_WES_TES_WDL_BASH_LAUNCH`**, **`FERRUM_WES_TES_WORK_HOST_PREFIX`**, and TES **`FERRUM_TES_DOCKER_*`** env vars (see [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md)). |

---

## Related docs

- [WORKFLOWS.md](WORKFLOWS.md) — user-oriented run flow, logs, engines.
- [GA4GH.md](GA4GH.md) — WES paths and auth.
- [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) — CI conformance coverage.

---

*[← Documentation index](README.md)*
