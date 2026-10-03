# Ferrum + ga4gh-infra integration

When Ferrum is deployed alongside [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra), the identity/access plane moves to ga4gh-infra; Ferrum focuses on the data/compute plane.

## Ownership

| Concern | Owner when co-deployed |
|---------|------------------------|
| Passport broker, visa issuance | ga4gh-infra `aai-broker` + `visa-registry` |
| Passport/visa validation | `ga4gh-clearinghouse` (via Ferrum `[auth] clearinghouse = true`) |
| DUO matching, ADS introspect | ga4gh-infra |
| Service discovery | ga4gh-infra `service-registry` |
| DRS, WES, TES, TRS, Beacon, htsget | Ferrum |

## Ferrum configuration (external auth)

```toml
[auth]
mode = "external"
require_auth = true
issuer = "http://localhost:8180"
# Optional override. When unset, external mode reads `{issuer}/.well-known/openid-configuration`
# and uses `jwks_uri` only when its scheme matches the issuer. ADR-028.
# jwks_url = "http://localhost:8180/jwks.json"
clearinghouse = true
ads_url = "http://localhost:8190"

[discovery]
enabled = true
service_registry_url = "http://localhost:8183"
auto_register = true
registration_base_url = "http://ferrum-gateway:8080"
# Re-POST this often. Set the registry stale_after_seconds to at least twice this value (600).
heartbeat_interval_secs = 300

[services]
enable_passports = false
```

Environment equivalents:

```bash
export FERRUM_AUTH__ISSUER=http://localhost:8180
# FERRUM_AUTH__JWKS_URL is optional when the issuer publishes OIDC discovery.
export FERRUM_DISCOVERY__ENABLED=true
export FERRUM_DISCOVERY__SERVICE_REGISTRY_URL=http://localhost:8183
export FERRUM_DISCOVERY__REGISTRATION_BASE_URL=http://ferrum-gateway:8080
export FERRUM_SERVICES__ENABLE_PASSPORTS=false
```

Minimal external bootstrap is the database, storage, which routes are on, `FERRUM_AUTH__ISSUER`, the registry URL, the registration key, and `FERRUM_DISCOVERY__REGISTRATION_BASE_URL`. The JWKS URL is the override. Recommended registry liveness is a 300 second heartbeat and `stale_after_seconds` of at least 600. A graceful stop deletes only the services this process has enabled. A kill leaves the row.

## Co-deploy Docker build (monorepo context)

The `external-auth` feature pulls `ga4gh-clearinghouse` via a path dependency. Co-deploy images use
`Ferrum/deploy/Dockerfile.gateway-monorepo` with build context = parent directory containing both
`Ferrum/` and `ga4gh-infra/` (e.g. `SynapticFour/`). Standalone `deploy/Dockerfile.gateway` builds
do not include `external-auth` unless you provide the sibling `ga4gh-infra` checkout or publish
`ga4gh-clearinghouse` to a registry and switch the dependency in `ferrum-core/Cargo.toml`.

```bash
docker build -f Ferrum/deploy/Dockerfile.gateway-monorepo \
  --build-arg FERRUM_GATEWAY_FEATURES="full,tes-docker,external-auth" \
  -t ferrum-gateway:co-deploy \
  /path/to/SynapticFour
```

See also [DECISIONS.md](../DECISIONS.md) (ADR-017) and ga4gh-infra [docs/DECISIONS.md](../../ga4gh-infra/docs/DECISIONS.md).

## Standalone mode (unchanged)

Without `mode = "external"`, Ferrum continues to use built-in `ferrum-passports` and config-based service URLs.

## Deployment entry points

- **Ferrum-Lab-Kit:** `lab-kit generate compose --with-ga4gh-infra` or profile `field-edge+infra`
- **Ferrum-GA4GH-Demo:** `./run --with-infra`
- **HelixTest:** `helixtest --all --mode ferrum+infra`

## Port matrix (co-deploy)

| Service | Port |
|---------|------|
| Ferrum gateway | 8080 |
| ga4gh-infra broker | 8180 |
| visa-registry | 8181 |
| duo-service | 8182 |
| service-registry | 8183 |
| ADS | 8190 |
| mock-idp | 9100 |
