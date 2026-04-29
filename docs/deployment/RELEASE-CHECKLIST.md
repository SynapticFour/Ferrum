# Release Checklist (10 Required Checks)

Before each release/hotfix:

1. [ ] Scope and change ticket confirmed.
2. [ ] Target versions pinned (binary/image/chart).
3. [ ] `./scripts/docs_consistency_check.sh` passed.
4. [ ] `./scripts/deployment_preflight.sh --scenario <target>` passed.
5. [ ] Backup completed (DB/storage/config).
6. [ ] Security-sensitive config reviewed.
7. [ ] Staging rollout successful.
8. [ ] Production smoke tests defined and executed.
9. [ ] Rollback path validated.
10. [ ] Approval + communication completed.

Use together with `docs/deployment/UPDATE-SOP.md`.

