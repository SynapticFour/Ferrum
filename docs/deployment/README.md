# Deployment Scenarios

This page is the entrypoint for Ferrum deployment paths.

Ferrum's public status is early use. No clinical pilot deployment is recorded in this repository. The shapes below are design targets for how a deployment can be laid out. They are not a record of a production cluster or an institutional workload in use.

## Scenario Matrix

| Scenario | Recommended path | Internet required during install | Main doc |
|---|---|---|---|
| Local demo / evaluation | `ferrum demo start --edge` or `deploy/docker-compose.yml` | Optional (Edge mode: no) | `docs/AFRICA-DEPLOYMENT.md` (Edge) · `docs/INSTALLATION.md` (Docker) |
| Single-node (bare metal / VM), a design target | Binary + systemd + external Postgres/S3 | Optional | `docs/INSTALLATION.md` |
| Distributed HPC, a design target | Ansible inventory + playbooks | Usually yes | `docs/INSTALLATION.md` |
| Kubernetes (AKS/on-prem), a design target | Helm chart in `deploy/helm/` | Usually yes | `docs/INSTALLATION.md` |
| Air-gapped / isolated environment, a design target | Offline bundle import + pinned artifacts | No (target) | `docs/deployment/OFFLINE-AIRGAP.md` |

## Update and Bugfix Strategy

Recommended channels. Each channel is a design target for how updates could be delivered. None of them records a cluster that is in use.

- **Stable:** a design target for clusters and institutional workloads. It is not a statement that such a cluster is in use.
- **Fast:** a faster channel for evaluation environments. Not a clinical pilot.
- **Patch-only:** security and bugfix updates only.

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
./scripts/deployment_preflight.sh --scenario edge
./scripts/deployment_preflight.sh --scenario single-node
./scripts/deployment_preflight.sh --scenario hpc
./scripts/deployment_preflight.sh --scenario kubernetes
./scripts/deployment_preflight.sh --scenario offline
```

Operational templates:
- Update SOP: `docs/deployment/UPDATE-SOP.md`
- Release checklist: `docs/deployment/RELEASE-CHECKLIST.md`

## Edge mode / Offline Deployment

For intermittent connectivity and shared laptops, use **Edge mode** (SQLite + local storage):

```bash
ferrum demo start --edge
```

Full guide (one-command start, RAM/disk by platform, Raspberry Pi, macOS, Linux): [docs/AFRICA-DEPLOYMENT.md](../AFRICA-DEPLOYMENT.md)

