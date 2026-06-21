# Deprecated laptop aliases (Phase 7.5)

Per [ADR-018](../DECISIONS.md), **Edge mode** replaced **Laptop mode** in user-facing terminology. Aliases remain for **one deprecation cycle**; removal planned for **v0.3.0** (next major).

## Still supported (warn on use)

| Alias | Replacement |
|-------|-------------|
| Cargo feature `laptop` | `edge` |
| Profile `release-laptop` | `release-edge` |
| `./scripts/build-laptop-native.sh` | `./scripts/build-edge-native.sh` |
| `deploy/scripts/ci-laptop-demo-e2e.sh` | `ci-edge-demo-e2e.sh` (wrapper exec) |
| `make laptop` | `make edge` |
| CLI `--offline` | `--edge` (both work) |
| `i18n::laptop_start` | `i18n::edge_start` |
| `[profile.release-laptop]` in root Cargo.toml | `release-edge` |

## Documentation only (no runtime alias)

References to “shared lab laptop” as **hardware** remain valid in [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md) — that describes physical machines, not the old product name.

## Removal checklist (v0.3)

- [ ] Drop `laptop` Cargo feature from `ferrum-gateway`
- [ ] Remove `release-laptop` profile from `Cargo.toml`
- [ ] Delete `build-laptop-native.sh` wrapper
- [ ] Remove `ci-laptop-demo-e2e.sh` wrapper
- [ ] Rename embed test `test_laptop_mode_*` → `test_edge_mode_*`
- [ ] Update Ferrum-Lab-Kit to drop any `laptop` profile ids
