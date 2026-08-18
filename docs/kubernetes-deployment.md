# Kubernetes deployment (optional)

Ferrum ships a Helm chart under `deploy/helm/`. It is **optional** — the standard path is Docker Compose (`./install.sh`). The chart is versioned with the Ferrum release tag (**0.3.2** / image `ghcr.io/synapticfour/ferrum:v0.3.2`). There is no separate Helm release cycle. Defaults include a ServiceAccount, pod security context, `/health` and `/ready` probes, and `FERRUM_SERVICES__ENABLE_*` flags. Lab Kit `generate helm` writes the same storage/WES/TES adapter env onto its companion chart.

## When to use Helm

Use Helm if your organization runs Ferrum on an existing Kubernetes cluster (HPC, cloud, or on-prem) and prefers `helm upgrade` over Compose.

## Prerequisites

- Kubernetes 1.26+
- Helm 3.10+
- PostgreSQL and object storage (S3-compatible) reachable from the cluster
- Optional: OIDC provider (e.g. Keycloak) for authenticated deployments

## Quick start

1. Copy the example values and set passwords/URLs:

   ```bash
   cp deploy/helm/values.yaml.example my-values.yaml
   # Edit postgres, storage, auth.jwks_url. Image tag defaults to v0.3.2.
   ```

2. Install or upgrade:

   ```bash
   helm upgrade --install ferrum deploy/helm -f my-values.yaml
   ```

3. Verify:

   ```bash
   kubectl get pods -l app.kubernetes.io/name=ferrum
   curl -sf http://<service-or-ingress>/health
   ```

## Release bundle

GitHub Releases include `ferrum-helm-<tag>.tgz` and the same chart inside `ferrum-offline-<tag>.tar.gz` under `helm/`. Verify with `SHA256SUMS.txt` before use.

## Configuration mapping

| Compose / `.env.example` | Helm (`values.yaml.example`) |
|--------------------------|------------------------------|
| `FERRUM_VERSION` | `image.tag` |
| `POSTGRES_*` | `postgres.*` / `config.database.url` |
| `MINIO_*` | `storage.s3_*` / `config.storage.*` |
| `KEYCLOAK_JWKS_URL` | `auth.jwks_url` / `config.auth.jwks_url` |
| `GATEWAY_PORT` | `service.port` |

See `deploy/helm/values-local.yaml` and `values-production.yaml` for environment-specific starting points.

## Support

For Compose-first installation and updates, see [customer-runbook.md](./customer-runbook.md).
