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

## WES → TES (Ferrum defaults)

WES can submit runs to TES with **engine-specific** images and command vectors (`ferrum-wes` TES backend). Assumptions are **best-effort** (public images, default entrypoints). Operators should validate **`entrypoint` / `command`** for their images and pin versions. For Cromwell / cwltool / Nextflow / Snakemake, expect different base images and CLI shapes; adjust TES task JSON or extend WES configuration when defaults are insufficient.

---

## DRS: metadata vs bytes (cross-reference)

For downloads, distinguish:

- **`GET /ga4gh/drs/v1/objects/{id}/access/{access_id}`** — JSON **`AccessUrl`** (URL to fetch, headers, optional expiry); not the object body.
- **`GET /ga4gh/drs/v1/objects/{id}/stream`** — **raw bytes** (plaintext or Crypt4GH path per config).

`access_url` in the database is JSONB and may be a **string** or **`{"url": "…"}`**; Ferrum read paths accept both.

---

*[← Documentation index](README.md)*
