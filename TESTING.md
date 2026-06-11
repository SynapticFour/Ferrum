# Testing

Ferrum uses a layered test strategy:

- crate-level unit and integration tests (`cargo test --workspace --all-targets`)
- lint and formatting gates (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`)
- GA4GH conformance and cross-service checks in CI via HelixTest

## Local verification

Before opening a PR, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Laptop Mode integration tests and offline E2E:

```bash
cargo test -p ferrum-embed
sh deploy/scripts/ci-laptop-demo-e2e.sh
```

See [docs/AFRICA-DEPLOYMENT.md](docs/AFRICA-DEPLOYMENT.md) for the one-command user path (`ferrum demo start --offline`).

## Conformance testing

The repository includes dedicated workflows for standards conformance. See:

- [`docs/HELIXTEST-INTEGRATION.md`](docs/HELIXTEST-INTEGRATION.md)
- [`.github/workflows/conformance.yml`](.github/workflows/conformance.yml)

## Contribution expectation

Behavioral changes should include tests. If tests are not practical, document the rationale in the PR.
