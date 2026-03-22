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
| **Symmetric host-path** bind for WES → TES | **Opt-in** — set **`FERRUM_WES_TES_WORK_HOST_PREFIX`** on the **gateway** (see below). No effect unless set. |
| **TES `volumes[]` → Docker `HostConfig.Binds`** | **Implemented** for the **Docker** TES backend (strings `host:container[:opts]` or objects `{ "host", "container", "mode?" }`). |
| **Optional socket / CLI / network / platform** | **Opt-in** env on the **TES** process (see below). |

---

## Gateway image: Docker-backed TES (`tes-docker` feature)

The default **`ferrum-gateway`** binary does **not** enable Bollard. To run **`FERRUM_TES_BACKEND=docker`** (or equivalent deployment choice), build with:

```bash
cargo build -p ferrum-gateway --features tes-docker
```

Docker image:

```bash
docker build -f deploy/Dockerfile.gateway --build-arg FERRUM_GATEWAY_FEATURES=tes-docker -t ferrum-gateway:tes-docker ..
```

Without this feature, choosing the `docker` TES backend falls back to **Podman** (existing behaviour).

---

## TES Docker executor: optional environment (site-specific)

All variables are **ignored unless set**. They apply only when the TES backend is **Docker** (Bollard).

| Variable | Effect |
|----------|--------|
| **`FERRUM_TES_DOCKER_MOUNT_SOCKET`** | If truthy (`1`, `true`, `yes`, `on`), add bind `/var/run/docker.sock:/var/run/docker.sock`. |
| **`FERRUM_TES_DOCKER_CLI_HOST_PATH`** | Host path to a static **`docker`** binary; bind-mount read-only into the task. Target path: **`FERRUM_TES_DOCKER_CLI_CONTAINER_PATH`** (default `/usr/local/bin/docker-host`). |
| **`FERRUM_TES_DOCKER_NETWORK_MODE`** | Docker `NetworkMode` (e.g. `host`, `bridge`, or a custom network name). |
| **`FERRUM_TES_DOCKER_EXTRA_HOSTS`** | Comma-separated `hostname:ip` entries (e.g. `host.docker.internal:host-gateway`). |
| **`FERRUM_TES_DOCKER_PLATFORM`** | Container create **platform** (API ≥ 1.41), e.g. `linux/amd64` for arm64 hosts pulling amd64-only images. |

**Security:** socket and host binds increase privilege; keep defaults off for untrusted tenants.

---

## WES → TES: optional environment (gateway)

Defaults are **unchanged** from older Ferrum (minimal `image` + `command`, no `volumes`). Enable extras only when your deployment needs them.

| Variable | Effect |
|----------|--------|
| **`FERRUM_WES_TES_WORK_HOST_PREFIX`** | Absolute host directory prefix. Ferrum adds a TES volume `{prefix}/{run_id}:{prefix}/{run_id}:rw` so nested `docker run -v` can resolve the **same** path on the host. Align **`FERRUM_WES_WORK_DIR`** / `work_dir_base` with this layout. |
| **`FERRUM_WES_TES_CONTAINER_WORKDIR`** | Sets **`executors[].workdir`** on submitted tasks (optional working directory inside the task). |
| **`FERRUM_WES_TES_WDL_BASH_LAUNCH`** | If truthy and **`workflow_type`** is WDL: use **`/bin/bash -lc`** and run Cromwell with **`$FERRUM_WES_WORKFLOW_URL`**; optional **`inputs.json`** in the work dir (from **`workflow_params`** when non-empty). |
| **`FERRUM_WES_TES_NEXTFLOW_FILE_LAUNCH`** | If truthy and type is Nextflow: download workflow with **`curl`**, write minimal **`nextflow.config`** (`docker { enabled = true }`), run **`nextflow run workflow.nf`** with **`-params-file params.json`** when **`workflow_params`** was written. Requires the work dir to be visible in the task (same bind pattern as above). |

Workflow URL is always passed as env **`FERRUM_WES_WORKFLOW_URL`** in those opt-in modes.

---

## WES → TES (Ferrum defaults)

With **no** env vars above, WES submits the same **minimal** tasks as before (suitable for CI and simple demos). For JVM **`ENTRYPOINT`** images (e.g. Cromwell) the legacy argv shape may still be wrong for some images — use **`FERRUM_WES_TES_WDL_BASH_LAUNCH`** or custom TES clients that set **`entrypoint`** / **`volumes`** explicitly.

---

## DRS: metadata vs bytes (cross-reference)

For downloads, distinguish:

- **`GET /ga4gh/drs/v1/objects/{id}/access/{access_id}`** — JSON **`AccessUrl`** (`url`, optional `headers`, optional `expires_at`); **not** the object body. For **S3/MinIO**, `url` may be a **presigned** link when presigning is configured (fallback to stored URL if presign fails).
- **`GET /ga4gh/drs/v1/objects/{id}/stream`** — **raw bytes** (plaintext or Crypt4GH path per config), not JSON.

`access_url` in the database is JSONB and may be a **string** or **`{"url": "…"}`**; Ferrum read paths accept both. Details: [GA4GH.md — access vs stream](GA4GH.md#drs-get-accessaccess_id-vs-get-stream).

Many **`…/stream`** URLs share the same path basename (`stream`). If a workflow engine stages inputs **only by basename**, collisions are possible. Use **distinct local names** (e.g. Nextflow **`stageAs`**, or WDL **`File`** inputs mapped to unique paths). Ferrum does not rewrite client staging behaviour.

---

*[← Documentation index](README.md)*
