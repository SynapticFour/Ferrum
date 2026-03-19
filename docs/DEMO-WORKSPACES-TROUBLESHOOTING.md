# Why "Failed to load workspaces" and why fixes didn't work

## What actually happens

1. You open the UI at **http://localhost:8082** (nginx).
2. The Workspaces page runs `fetch('/workspaces/v1/workspaces')` → the browser requests **http://localhost:8082/workspaces/v1/workspaces** (same origin).
3. **Nginx** receives that request and, via `location /workspaces/`, forwards it to the **ferrum-gateway** container.
4. The **gateway** must:
   - Have the `/workspaces/v1` route **mounted** (only if it was built/started with a DB pool).
   - Put **AuthClaims** (e.g. demo-user) on the request so the workspaces handler can run.

If any of this fails, the browser gets a non-2xx response and the UI shows "Failed to load workspaces". The UI did **not** show the real HTTP status or body, so we couldn’t see whether the problem was 404, 403, 500, or something else.

---

## Why it can fail (and what we tried)

| Cause | What you see | What we changed |
|-------|----------------|------------------|
| **Gateway has no workspaces route (404)** | Route not registered because `workspaces_pool` was `None`. | Create DB pool in gateway `main.rs` from config or `FERRUM_DATABASE__URL` and pass it to `run()` so the route is mounted. |
| **No AuthClaims → 403 or 500** | Workspaces handler needs `auth.sub()`; without token or demo-user it fails. | Auth middleware: when `require_auth` is false (or no config), inject synthetic "demo-user" claims. Gateway: always pass an auth config (from config or `AuthMiddlewareConfig::demo()`). |
| **Old gateway still running** | New code never runs. | You must **rebuild the gateway image** and **recreate the container** so the new binary is used. |

So far we fixed (1) and (2) in code. If the error is still there, the usual reason is **(3): the running container is still the old gateway**.

---

## Why "make rebuild" and "make demo" might not fix it

- **`make rebuild`** runs `docker compose build --no-cache`. That rebuilds **images** (including `ferrum-gateway`).
- **`make demo`** runs `docker compose up -d --build`. That starts **containers** from the current images.

So in theory the new image is used. In practice:

1. **Compose may reuse an existing container** if the service name and image tag haven’t changed. So the old container (old process) can keep running.
2. **Build cache** can still be used in some setups (e.g. BuildKit cache), so the image might not actually contain the new code.
3. You might be looking at a **cached frontend** (browser or nginx) and not hitting the new backend.

So we need to **force the new gateway to run** and then **see the real error** from the API.

---

## What to do step by step

### 1. See the real error (once)

The UI is now changed to show the **exact** error message (e.g. `HTTP 404` or the response body). After rebuilding the UI and opening the Workspaces page again, you should see that message. It tells us whether the gateway returns 404, 403, 500, or a body like "authentication required".

### 2. Force the new gateway to run

Run this from the **repo root** (where `deploy/` is):

```bash
# Stop and remove the gateway container (and nginx so it doesn't keep talking to old gateway)
docker compose -f deploy/docker-compose.yml stop ferrum-gateway nginx

# Remove the gateway image so the next build is from scratch
docker compose -f deploy/docker-compose.yml rm -f ferrum-gateway 2>/dev/null || true
docker rmi ferrum-gateway:latest 2>/dev/null || true

# Rebuild the gateway image (no cache)
docker compose -f deploy/docker-compose.yml build --no-cache ferrum-gateway

# Start everything again (creates new container from new image)
docker compose -f deploy/docker-compose.yml up -d

# Optional: check that the gateway process is running and which binary it is
docker compose -f deploy/docker-compose.yml exec ferrum-gateway ls -la /app/ 2>/dev/null || true
```

Then open **http://localhost:8082/workspaces** and check the **exact** error text shown under "Failed to load workspaces."

### 3. Check the API directly (no UI)

From your host (same machine as Docker):

```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:8082/workspaces/v1/workspaces
```

- **200** – API works; if the UI still fails, it’s likely cache or a different URL.
- **404** – Gateway doesn’t have the route (old binary or pool not passed).
- **403** – Auth/problem with claims (e.g. demo-user not injected).
- **502/503** – Nginx can’t reach gateway or gateway not up.

Also:

```bash
curl -v http://localhost:8082/workspaces/v1/workspaces
```

Shows the full response (headers + body). That body is what the UI will show as "Error: ..." now.

---

## Summary

- **Why it happens:** The workspaces API call fails (404 / 403 / 500 / connection). The UI used to only say "Failed to load workspaces" and hid the real reason.
- **Why previous fixes didn’t help:** The code changes (pool + auth/demo-user) are correct, but the **running gateway was still the old image/container**, so the new logic never ran.
- **What to do:** Rebuild the gateway image, **force recreate** the gateway (and nginx) container, then check the **exact** error in the UI and/or with `curl` as above. The new error text (and HTTP code) will tell us the next step if it still fails.
