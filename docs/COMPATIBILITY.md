# Compatibility & deprecation policy

**Product:** Ferrum
**Status:** 2026-08-12 · org level-up **D4**
**Audience:** operators pinning releases for pilots

This is **API / release** compatibility policy — not [LICENSE-COMPATIBILITY.md](../LICENSE-COMPATIBILITY.md) (dependency license allow-list).

---

## Versioning

- **SemVer** for release tags (`vMAJOR.MINOR.PATCH`). See [RELEASING](../RELEASING.md) if present / GitHub Releases.
- **MINOR/PATCH:** backward-compatible for documented GA4GH surfaces and stable config keys when practical.
- **MAJOR:** may remove deprecated aliases or break config/API — announced in CHANGELOG.

## Deprecation window

1. Mark deprecated in docs / CHANGELOG with replacement.
2. Keep deprecated path at least **one minor** (prefer one major) unless a security issue forces removal.
3. Example: Edge rename / `laptop` alias — see DECISIONS.md ADR-018 area.

## BUSL Change Date

Each **version** of Ferrum is BUSL-1.1 with **Change License Apache-2.0** effective **four years** from that version’s release date (see [LICENSE](../LICENSE), [BUSINESS-MODEL.md](BUSINESS-MODEL.md)). Delayed open source reduces long-term lock-in; it is **not** a promise that APIs remain identical across majors.

## Operator guidance

| Do | Don't |
|----|-------|
| Pin image digest or release tag for pilots | Track `main` or floating `:latest` in production |
| Read CHANGELOG before upgrading | Assume silent compatible upgrades across MAJOR |
| Keep staging ≈ production pins | Skip backup before upgrade ([disaster recovery](https://github.com/SynapticFour/SynapticFour-Showcase/blob/main/docs/for-customers/disaster-recovery.md)) |

## Support alignment

Paid support upgrade assistance: [Showcase support-tiers](https://github.com/SynapticFour/SynapticFour-Showcase/blob/main/docs/for-customers/support-tiers.md) · business `delivery/support-sla.md`.
