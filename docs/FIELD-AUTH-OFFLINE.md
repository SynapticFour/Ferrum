# Field Edge — auth & long offline (Phase 3)

Operator guide for shared Edge devices with intermittent connectivity.

## Field roles (Passport visas)

| Visa | Role | Typical use |
|------|------|-------------|
| `ferrum:collector` | Field collector | ONT ingest, metadata capture |
| `ferrum:analyst` | Analyst | Beacon queries, read-only analysis |
| `ferrum:sync_operator` | Sync operator | `ferrum sync push` (Phase 4) |
| `ferrum:admin` | Admin | Full gateway access |

With ga4gh-infra co-deploy, issue visas via the broker. For **shared devices without network**, use local Edge accounts:

```bash
# One-time setup (requires auth.jwt_secret in config)
ferrum auth account add --username alice --role collector --pin '****'
ferrum auth login --username alice --pin '****'   # prints Bearer token

curl -H "Authorization: Bearer $TOKEN" \
  -F ont_metadata='{"format":"pod5",...}' \
  -F file=@run.pod5 \
  http://127.0.0.1:8080/api/v1/ingest/ont
```

Set `require_auth = true` in config to enforce roles on ingest.

## JWKS offline (7-day cache + file fallback)

Default JWKS cache TTL is **7 days** (`auth.jwks_cache_ttl_seconds = 604800`).

For field nodes without reliable internet:

```toml
[auth]
mode = "external"
require_auth = true
issuer = "https://broker.example.org"
jwks_file = "/home/pi/.ferrum/jwks/broker-2026.json"
jwks_cache_ttl_seconds = 604800
```

Env overrides: `FERRUM_AUTH__JWKS_FILE`, `FERRUM_AUTH__JWKS_CACHE_TTL_SECS`.

### Rotation without network

1. Pre-provision the next JWKS JSON on a USB stick or signed update bundle.
2. Install with `ferrum update install --bundle edge-update.tar.gz --jwks-dir ~/.ferrum/jwks`.
3. Update `jwks_file` in config to the new key id path (or symlink `active_kid`).
4. Restart gateway — validation uses the local file; no HTTP fetch required.

See [DECISIONS.md](../DECISIONS.md) ADR-020.

## Clock integrity

`GET /health` includes a `clock` object (NTP probe, skew warning). When skew exceeds `[africa] clock_max_skew_secs` (default 300s), status becomes `degraded`.

Before exporting residency audit chains offline, verify `clock.ntp_reachable` or confirm time manually.

## Update bundles with JWKS

```bash
ferrum update pack --gateway ./ferrum-gateway --output edge-bundle.tar.gz \
  --jwks broker-2026:/path/jwks.json --active-jwks-kid broker-2026
```

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [GA4GH-INFRA-INTEGRATION.md](GA4GH-INFRA-INTEGRATION.md), [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md).
