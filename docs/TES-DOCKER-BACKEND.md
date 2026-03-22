# TES backends: Docker, Podman, nested containers

Operational notes for **Ferrum TES** when tasks run under **Docker** (Bollard) or **Podman**, especially workflow engines that spawn **child containers** (`docker run` / `podman run` inside the task).

---

## Entrypoint vs command (Docker API semantics)

Many images set **`ENTRYPOINT`** (JVM images, wrapper scripts). In the Docker Engine API, **`Cmd`** is appended as extra argv to **`Entrypoint`** unless you override entrypoint.

- If you only set **`executors[].command`** in TES, Ferrum passes it as Docker **`Cmd`**; the image **`ENTRYPOINT`** remains.
- To run a **shell one-liner** reliably, either:
  - Set **`executors[].entrypoint`** to e.g. `["/bin/bash", "-lc"]` and put the script in **`command`** (one string per argv after `-lc`), or
  - Use `["/bin/sh", "-c", "your pipeline"]` (note: `/bin/sh` may be **dash** on Debian/Ubuntu — no `pipefail`; prefer **`bash`** when you need bash semantics).

Ferrum’s TES type **`TesExecutor`** exposes optional **`entrypoint`**; **`command`** follows Docker’s “argv after entrypoint” rules. **Podman** executor uses `--entrypoint <first>` then image, then remaining entrypoint args, then `command` (aligned with `podman run` ordering).

---

## Host bind mounts and `docker.sock`

If the task container mounts **`-v /host/path:/work`** and then runs **nested** `docker run -v /container/path:/work`, the **inner** daemon resolves paths on the **host**. A volume source that only exists inside the outer container often **does not** exist on the host → broken mounts.

Mitigations (choose per site):

- Use the **same absolute host path** on both sides of the outer container (e.g. `-v /data/run:/data/run`) so nested runs see a valid host path, or
- Use a **Docker volume** / named volume API instead of host bind for shared data, or
- Document a **site-specific** layout (Ferrum does not guess cluster paths).

---

## `docker.sock` vs `docker` CLI

Mounting **`/var/run/docker.sock`** exposes the daemon API. Engines or wrappers that invoke the **`docker` binary** still need a **compatible CLI** inside the task image (or a bind-mounted static client). The **API version** negotiated by the client must match the daemon; mismatch surfaces as client errors, not Ferrum HTTP errors.

---

## Long-running tasks and larger workdirs (Docker / Podman TES)

For **benchmarks** or **WES → TES** pipelines (Nextflow, WDL, …) that run longer than default container assumptions:

| Concern | What to configure |
|---------|-------------------|
| **Scratch / workdir size** | TES **`volumes`** / **`disk_gb`** (see your TES client): bind a **host directory** with enough space into the task (e.g. `/work`), or use a **Docker volume** with sufficient quota. Ferrum does not auto-grow anonymous container storage. |
| **Timeouts** | **TES** executor and **reverse proxies** may impose HTTP or idle timeouts; raise limits for long `stdout` or idle CPU phases. |
| **Nested engines** | Nextflow/Cromwell may spawn many processes; ensure **CPU / memory** limits on the Docker/Podman daemon (or Slurm allocation) match the workflow. |
| **WES work dir** | Non-TES WES executors use a **local** `work_dir_base` per run (see deployment config / `INSTALLATION.md`). Keep that filesystem on fast, spacious storage for large intermediates. |

Details and nested-mount pitfalls: [Nested container execution / Host path contract](#nested-container-execution--host-path-contract-wes--tes). WES engine matrix: **[WES-WORKFLOW-ENGINES.md](WES-WORKFLOW-ENGINES.md)**.

---

## Nested container execution / Host path contract (WES → TES)

**Problem:** WES submits TES tasks that may run **nested** `docker`/`podman` (see [Host bind mounts and `docker.sock`](#host-bind-mounts-and-dockersock)). A second, **orthogonal** issue is **volume strategy**: should the executor see the **same host path** for workflow scratch and engine mounts as the outer TES agent, or only a container-local path such as `/work`?

| Topic | Status in Ferrum (this repo) |
|--------|----------------------------|
| TES **entrypoint** optional (`executors[].entrypoint`) for Docker / Podman / Slurm-wrapped Podman | **Implemented** (see [Entrypoint vs command](#entrypoint-vs-command-docker-api-semantics)) |
| Docs on nested mounts / `docker.sock` | **Documented** (this file) |
| **Symmetric host-path** vs **“`/work` only”** volume contract between **WES → TES** task JSON and site bind mounts | **Open / documented-only** — no dedicated WES→TES volume normalisation or env-driven “host prefix” in code yet; operators must align TES `volumes` / WES defaults with their cluster layout. |

Explicit **backlog-style** reference: choosing a single supported pattern (e.g. `FERRUM_TES_WORK_HOST_PATH` or documented “always mount host `X` at `/work`”) would be a follow-up; this iteration does **not** add that configuration.

---

## WES → TES (Ferrum defaults)

WES can submit runs to TES with **engine-specific** images and command vectors (`ferrum-wes` TES backend). Assumptions are **best-effort** (public images, default entrypoints). Operators should validate **`entrypoint` / `command`** for their images and pin versions. For Cromwell / cwltool / Nextflow / Snakemake, expect different base images and CLI shapes; adjust TES task JSON or extend WES configuration when defaults are insufficient.

---

## DRS: metadata vs bytes (cross-reference)

For downloads, distinguish:

- **`GET /ga4gh/drs/v1/objects/{id}/access/{access_id}`** — JSON **`AccessUrl`** (`url`, optional `headers`, optional `expires_at`); **not** the object body. For **S3/MinIO**, `url` may be a **presigned** link when presigning is configured (fallback to stored URL if presign fails).
- **`GET /ga4gh/drs/v1/objects/{id}/stream`** — **raw bytes** (plaintext or Crypt4GH path per config), not JSON.

`access_url` in the database is JSONB and may be a **string** or **`{"url": "…"}`**; Ferrum read paths accept both. Details: [GA4GH.md — access vs stream](GA4GH.md#drs-get-accessaccess_id-vs-get-stream).

---

*[← Documentation index](README.md)*
