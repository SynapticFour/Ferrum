# WES workflow engines (WDL, Nextflow, CWL, Snakemake)

Ferrum **WES** routes runs to an executor from **`workflow_type`** (and optional **`workflow_engine_params`**) on the run request. This matches common **GA4GH WES** expectations: clients submit **`workflow_type` + `workflow_url`** (and attachments).

---

## Supported `workflow_type` values (Ferrum)

| `workflow_type` (case-insensitive) | Direct executor (no TES) | When **`FERRUM_WES_TES_URL`** / TES is configured |
|-----------------------------------|---------------------------|---------------------------------------------------|
| **`wdl`** | Cromwell-style command (see `CromwellExecutor`) | Default: Cromwell image + `java -jar … run <url>`. Optional **`FERRUM_WES_TES_WDL_BASH_LAUNCH`** → shell + `inputs.json` (see [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md)). |
| **`nextflow`** or **`nxf`** | `NextflowExecutor` | Default: `nextflow run <workflow_url>`. Optional **`FERRUM_WES_TES_NEXTFLOW_FILE_LAUNCH`** → download + local `nextflow.config` (see TES doc). |
| **`cwl`** | `cwltool` | TES: **`quay.io/commonwl/cwltool:latest`** |
| **`snakemake`** | `snakemake` | TES: **`snakemake/snakemake:latest`** |

**Source:** `crates/ferrum-wes/src/run_manager.rs` (`executor_for_type`), `crates/ferrum-wes/src/executors/tes.rs` (`build_tes_task_request`, `legacy_image_and_command`).

**No fork required for Nextflow** — submit WES with `workflow_type: "Nextflow"` (or `nextflow` / `nxf`) and a **`workflow_url`** pointing at your script (e.g. TRS URL, `https://`, or `file:` where your deployment allows it). TES must be reachable and the **task image** must contain a working **Nextflow** install (default public image above).

---

## `workflow_engine_params` (examples)

| Key | Effect |
|-----|--------|
| **`ferrum_backend`** / **`ferrum-backend`** | Value **`slurm`** forces **Slurm** executor when TES is **not** configured (see `run_manager.rs`). |

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
