# Update SOP Template

Use this template for controlled updates and bugfix rollouts.

## 1) Change metadata

- Change ID:
- Date/time:
- Owner:
- Environment: (demo/single-node/hpc/kubernetes/offline)
- Target versions:
  - ferrum-gateway:
  - helm chart (if applicable):
  - infra/playbook version:
- Change type: (security fix / bugfix / minor / major)

## 2) Risk and scope

- Affected components:
- Expected impact:
- Maintenance window:
- Rollback target:
- Abort criteria:

## 3) Go/No-Go checks

- [ ] `./scripts/deployment_preflight.sh --scenario <scenario>` passed
- [ ] Backup created (DB + storage + config)
- [ ] Secrets/config reviewed
- [ ] Pinned versions confirmed (no unreviewed latest)
- [ ] Stakeholders informed

## 4) Rollout execution

- [ ] Deployment command executed
- [ ] Logs reviewed
- [ ] Health endpoint green

## 5) Validation

- [ ] `/health`
- [ ] DRS endpoint smoke test
- [ ] WES/TES smoke test (if enabled)
- [ ] Auth flow test (if enabled)

## 6) Rollback (if needed)

- [ ] Rollback executed
- [ ] Previous version restored
- [ ] Post-rollback health green

## 7) Post-update

- [ ] Change log updated
- [ ] Ticket linked
- [ ] Lessons learned recorded

