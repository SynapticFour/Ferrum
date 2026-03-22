# WES workflow engines (WDL, Nextflow, CWL, Snakemake)

Ferrum **WES** routes runs to an executor from **`workflow_type`** (and optional **`workflow_engine_params`**) on the run request. This matches common **GA4GH WES** expectations: clients submit **`workflow_type` + `workflow_url`** (and attachments).

---

## Supported `workflow_type` values (Ferrum)

| `workflow_type` (case-insensitive) | Direct executor (no TES) | When **`FERRUM_WES_TES_URL`** / TES is configured |
|-----------------------------------|---------------------------|---------------------------------------------------|
| **`wdl`** | Cromwell-style command (see `CromwellExecutor`) | TES task uses **Cromwell image** + `run` (see `TesExecutorBackend::default_image_and_command`) |
| **`nextflow`** or **`nxf`** | `NextflowExecutor` | TES task uses **`nextflow/nextflow:latest`** + `nextflow run <workflow_url>` |
| **`cwl`** | `cwltool` | TES: **`quay.io/commonwl/cwltool:latest`** |
| **`snakemake`** | `snakemake` | TES: **`snakemake/snakemake:latest`** |

**Source:** `crates/ferrum-wes/src/run_manager.rs` (`executor_for_type`), `crates/ferrum-wes/src/executors/tes.rs` (`default_image_and_command`).

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
| **Custom Nextflow images / JVM flags** | Today TES path uses **default** image + argv; production often needs **site-specific** TES task JSON or extended WES params — track as deployment feature if you need arbitrary `container_image` per run. |
| **WDL parity** | Same pattern as Nextflow: `workflow_type: "WDL"` + `workflow_url`; override behaviour via TES task templates when Ferrum exposes them. |

---

## Related docs

- [WORKFLOWS.md](WORKFLOWS.md) — user-oriented run flow, logs, engines.
- [GA4GH.md](GA4GH.md) — WES paths and auth.
- [HELIXTEST-INTEGRATION.md](HELIXTEST-INTEGRATION.md) — CI conformance coverage.

---

*[← Documentation index](README.md)*
