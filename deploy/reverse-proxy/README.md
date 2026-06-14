# Reverse proxy examples for pilot / self-hosted Ferrum

Copy-paste configs for putting a Ferrum demo stack behind TLS and an access gate (Basic Auth or IP allowlist).

| File | Use when |
|------|----------|
| [`Caddyfile.pilot.example`](Caddyfile.pilot.example) | Automatic TLS (Let's Encrypt), minimal config |
| [`nginx.pilot.conf.example`](nginx.pilot.conf.example) | Existing nginx + manual certificates |

**Upstream:** point at the demo stack UI/API proxy (`http://127.0.0.1:8082` after `ferrum demo start` or `make demo`), not the raw gateway port unless you intentionally skip the bundled nginx.

**Fly.io pilot reference:** [`synapticfour-business/customers/pasteur-tunis/pilot-deploy/ferrum/Caddyfile`](https://github.com/SynapticFour/synapticfour-business/blob/main/customers/pasteur-tunis/pilot-deploy/ferrum/Caddyfile) (production-shaped, env-var credentials).

See also `ga4gh-infra/docker/reverse-proxy/Caddyfile.example` for the identity plane (broker, visa-registry, etc.).
