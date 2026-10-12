# ferrum-ui authentication

How the Ferrum UI signs users in against a ga4gh-infra broker, and what the gateway exposes for it.

**Scope and honesty.** This document describes code paths that exist in this repository. It does not claim a clinical pilot, GA4GH certification, or a recorded laptop run. The compose overlay in [Laptop stack](#laptop-stack-full-infra--ga4gh-infra) is **unproven** until someone runs it and records the result.

## `GET /admin/config` (auth fields)

`GET /admin/config` is public and returns a sanitized config (no secrets). The UI reads the `auth` object to decide whether and where to offer sign-in. Source: `crates/ferrum-gateway/src/admin.rs` (`SanitizedAuth`). The route exists only in gateway builds that compile `/admin` (`full` and `full-infra`; **not** `edge` or `edge-infra`, see [IMAGE-VARIANTS.md](IMAGE-VARIANTS.md)).

| Field | Type | Meaning |
|-------|------|---------|
| `auth.mode` | `"builtin"` \| `"external"` | `external` = Passports minted by a ga4gh-infra broker; `builtin` = Ferrum-local tokens. |
| `auth.require_auth` | bool | Resolved `require_auth` from config (`FERRUM_AUTH__REQUIRE_AUTH` overrides the runtime check; default on). |
| `auth.broker_public_url` | string, optional | `FERRUM_PUBLIC_BROKER_URL`, else `auth.issuer`. Omitted when neither is set. |
| `auth.broker_login_url` | string, optional | `{broker_public_url}/login/{idp}` where `idp` is `FERRUM_BROKER_LOGIN_IDP` (default `keycloak`). Omitted when `broker_public_url` is unset. |
| `auth.access_requests_enabled` | bool | True when `auth.mode=external`, or `auth.ads_url` is set, or discovery is enabled. Tells the UI that ADS-backed dataset access requests are available. It is **not** a statement that the ADS is reachable. |

Example (external mode, local broker):

```json
{
  "auth": {
    "mode": "external",
    "require_auth": true,
    "broker_public_url": "http://localhost:8180",
    "broker_login_url": "http://localhost:8180/login/mock-idp",
    "access_requests_enabled": true
  }
}
```

If `broker_login_url` is absent the UI has nothing to redirect to. With `require_auth=true` the UI then shows a broker-misconfiguration state and offers no sign-in button; it does not guess a URL.

## Sign-in flow

The UI is **not** an OIDC confidential client. It holds no client secret, performs no code exchange, and does not talk to Keycloak.

1. UI reads `auth.broker_login_url` from `GET /admin/config`.
2. UI redirects the browser to `{broker_login_url}?return_url=https://<ui-host>/auth/callback` (URL-encoded). The path comes from the Vite `base` (`authCallbackPath()` in `services/ui/src/lib/auth.ts`); on a sub-path deployment it is `<base>/auth/callback`.
3. The broker authenticates the user against its configured upstream IdP (Keycloak, mock-idp, ...). Keycloak stays a **broker to IdP** concern.
4. The broker redirects back to `return_url` with the Passport in the URL **fragment** (`#access_token=...`). Fragments are not sent to servers.
5. `/auth/callback` parses the fragment (`parseTokenFromLocationHash`), stores the Passport in the browser, and sends it as `Authorization: Bearer <Passport>` on API calls.
6. The gateway validates the bearer: **issuer = the broker public URL** (`FERRUM_AUTH__ISSUER`), signature via the broker JWKS (`FERRUM_AUTH__JWKS_URL`), Passport visas via the clearinghouse (`full-infra` / `external-auth` build).

Constraints that follow from this:

- `FERRUM_AUTH__ISSUER` must equal the `iss` the broker puts in tokens, which is the **browser-visible** broker URL. `FERRUM_AUTH__JWKS_URL` may use an in-network hostname.
- `return_url` must be on a host the broker allows. Configure that on the broker side (ga4gh-infra), not in Ferrum.
- Anonymous catalog reads still work with `require_auth=true`; see [GA4GH.md](GA4GH.md#require_passport_for_bytes-adr-030).

## Related gateway behaviour (v0.3.4)

- `GET /api/v1/audit/residency` returns **401** without a bearer when `require_auth` is on (ADR-031). `GET /api/v1/audit/residency/verify` stays unauthenticated (chain metadata only).
- Invalid or revoked bearer tokens append `authorization_denied` events to the residency chain (ADR-032), rate-limited and with the client address minimized (`[audit]` config: `anonymous_client_ip` = `omit` default, `truncated`, `full`; `denied_events_per_minute`, default 30). Missing-bearer and DRS-level `forbidden` denials are **not yet** recorded; only the auth middleware path is wired.
- With HelixTest stubs enabled, WES and TES `service-info` carry an explicit `SIMULATED / NON-PILOT` marker (ADR-033).

## Laptop stack (`full-infra` + ga4gh-infra)

Target: one laptop, UI + gateway + Postgres + MinIO + ga4gh-infra broker.

| Piece | Source |
|-------|--------|
| Gateway image variant | `full-infra` (`FERRUM_VARIANT=full-infra`, [IMAGE-VARIANTS.md](IMAGE-VARIANTS.md)) |
| Postgres, MinIO, init, UI | `deploy/docker-compose.yml` |
| Broker, mock-idp, visa-registry, ADS, service-registry | `deploy/docker-compose.ga4gh-infra.yml` (needs `GA4GH_INFRA_SRC`) |
| Overlay for external auth | `deploy/docker-compose.full-infra-ui.yml` |

Required gateway environment (the overlay sets these):

```bash
FERRUM_AUTH__MODE=external
FERRUM_AUTH__REQUIRE_AUTH=true
FERRUM_AUTH__CLEARINGHOUSE=true
FERRUM_AUTH__ISSUER=http://localhost:8180          # = FERRUM_PUBLIC_BROKER_URL
FERRUM_AUTH__JWKS_URL=http://aai-broker:8080/jwks.json
FERRUM_PUBLIC_BROKER_URL=http://localhost:8180
FERRUM_BROKER_LOGIN_IDP=mock-idp                   # default: keycloak
# optional (ADR-030, default false):
FERRUM_AUTH__REQUIRE_PASSPORT_FOR_BYTES=true
```

Commands:

```bash
export GA4GH_INFRA_SRC=/abs/path/to/ga4gh-infra
docker compose \
  -f deploy/docker-compose.yml \
  -f deploy/docker-compose.ga4gh-infra.yml \
  -f deploy/docker-compose.full-infra-ui.yml \
  up -d --build
```

Checks to run and record before anyone calls this working:

```bash
curl -s http://localhost:8080/admin/config | jq .auth
# expect mode=external, require_auth=true, non-empty broker_login_url
curl -si http://localhost:8080/api/v1/audit/residency | head -1      # expect 401
curl -s  http://localhost:8080/api/v1/audit/residency/verify | jq .  # expect 200 chain metadata
curl -s  http://localhost:8080/ga4gh/wes/v1/service-info | jq '.description, .tags'
# stubs off (overlay) -> no "SIMULATED" marker
```

Status of this repository's claim: `docker compose config` accepts the merged files. No stack was started and no UI sign-in was exercised when this document was written.

Existing, separately maintained laptop pilot paths (`deploy/docker-compose.pilot.yml`, `deploy/docker-compose.pilot-cloud.yml`) remain the reference for the Fly AAI and mock-idp pilots.
