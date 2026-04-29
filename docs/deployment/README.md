# Deployment Scenarios

This page is the entrypoint for Ferrum deployment paths.

## Scenario Matrix

| Scenario | Recommended path | Internet required during install | Main doc |
|---|---|---|---|
| Local demo / evaluation | `ferrum demo start` or `deploy/docker-compose.yml` | Yes | `docs/INSTALLATION.md` |
| Single-node production (bare metal / VM) | Binary + systemd + external Postgres/S3 | Optional | `docs/INSTALLATION.md` |
| Distributed HPC | Ansible inventory + playbooks | Usually yes | `docs/INSTALLATION.md` |
| Kubernetes (AKS/on-prem) | Helm chart in `deploy/helm/` | Usually yes | `docs/INSTALLATION.md` |
| Air-gapped / isolated environment | Offline bundle import + pinned artifacts | No (target) | `docs/deployment/OFFLINE-AIRGAP.md` |

## Update and Bugfix Strategy

Recommended channels:
- **Stable:** production clusters and institutional workloads.
- **Fast:** pilot environments.
- **Patch-only:** security + bugfix updates only.

| Path | Delivery method | Rollback | Cadence |
|---|---|---|---|
| Local demo | pull latest CLI/images, restart demo | restart previous image tag / binary | ad hoc |
| Single-node | pinned binary/image versions, controlled restart | restore previous binary/image + restart | monthly + hotfix |
| HPC/Ansible | versioned playbook vars and staged rollout | re-run playbook with previous version vars | monthly + hotfix |
| Kubernetes | pinned chart/image tags + `helm upgrade` | `helm rollback` | monthly + hotfix |
| Air-gapped | signed offline bundles | re-import previous bundle | quarterly + security fixes |

## Preflight (recommended before every rollout)

```bash
./scripts/deployment_preflight.sh --scenario demo
./scripts/deployment_preflight.sh --scenario single-node
./scripts/deployment_preflight.sh --scenario hpc
./scripts/deployment_preflight.sh --scenario kubernetes
./scripts/deployment_preflight.sh --scenario offline
```

Operational templates:
- Update SOP: `docs/deployment/UPDATE-SOP.md`
- Release checklist: `docs/deployment/RELEASE-CHECKLIST.md`

