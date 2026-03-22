# Contributing to Ferrum

Thank you for your interest in contributing. This document covers our code of conduct, development setup, testing, PR process, adding a new GA4GH service, and Rust style.

---

## Who builds Ferrum

Ferrum is developed by Synaptic Four, a company based in Germany 🇩🇪 founded and staffed by individuals on the autism spectrum. We believe that the attention to detail, systematic thinking, and deep focus that characterizes our work makes us particularly well-suited to building infrastructure that researchers can rely on — tools that are precise, thoroughly documented, and that behave exactly as specified.

We welcome contributors from all backgrounds. We value clear communication, well-documented decisions, and thorough reviews over speed. If you have questions about our process or need accommodations to contribute comfortably, please reach out — we understand.

**License:** By contributing to Ferrum, you agree that your contributions will be licensed under the same **BUSL-1.1** terms as the rest of the project (see [LICENSE](LICENSE)). For how research vs commercial use fits the open-core model and how this relates to [Ferrum Lab Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit), see **[docs/BUSINESS-MODEL.md](docs/BUSINESS-MODEL.md)**. If you contribute on behalf of an employer, ensure you have **permission** to contribute under those terms.

**Not legal advice:** Repository docs do not replace counsel for your jurisdiction (employment, IP assignment, export rules, etc.).

---

## Code of conduct

We follow a respectful, inclusive code of conduct. Be kind and professional in issues, PRs, and discussions. Harassment or discriminatory behavior is not tolerated.

---

## Development setup

1. **Rust 1.75+**

   ```bash
   rustup update stable
   ```

2. **Clone and build**

   ```bash
   git clone https://github.com/SynapticFour/Ferrum
   cd Ferrum
   cargo build
   cargo test
   ```

3. **Pre-commit (optional)**

   - Run `cargo fmt` and `cargo clippy` before committing.
   - We recommend:
     - `cargo fmt -- --check` (or format on save in your editor)
     - `cargo clippy --all-targets --all -- -D warnings`

---

## Testing

- **Unit tests:** `cargo test --all`
- **Integration tests:** Same; integration tests live under `*/tests/` or within crates.
- **CI (Rust):** GitHub Actions runs `cargo test --all` and `cargo clippy --all-targets --all -- -D warnings` on push and PRs.
- **GA4GH conformance (HelixTest):** The [Conformance (HelixTest)](.github/workflows/conformance.yml) workflow builds the demo stack and runs [HelixTest](https://github.com/SynapticFour/HelixTest) in Ferrum mode. **What is covered in CI** (WES, TES, DRS, TRS, Beacon, htsget, E2E, etc.) is documented in [docs/HELIXTEST-INTEGRATION.md](docs/HELIXTEST-INTEGRATION.md).

---

## PR process and review

1. Open an issue or pick an existing one to discuss the change.
2. Fork, branch from `main`, and implement. Keep commits logical and messages clear.
3. Ensure tests pass and clippy is clean: `cargo test --all && cargo clippy --all-targets --all -- -D warnings`.
4. Open a PR against `main`. Fill in the PR template (if any); reference the issue.
5. Address review feedback. Maintainers will merge when approved and CI is green.

---

## Adding a new GA4GH service

Checklist for adding a new service crate (e.g. a new GA4GH API):

1. **Crate** — Add `crates/ferrum-<name>/` with `Cargo.toml` and dependency on `ferrum-core`.
2. **Router** — Expose a `router(state) -> Router` (or equivalent) that mounts routes under the standard path (e.g. `/ga4gh/<svc>/v1`).
3. **State** — Define an `AppState` (or use shared state) with DB pool, optional `Arc<dyn ferrum_storage::ObjectStorage>`, and any service-specific handles.
4. **Gateway** — In `ferrum-gateway`, add a config flag (e.g. `enable_<svc>`), construct state when enabled, and `app.nest("/ga4gh/<svc>/v1", ferrum_<svc>::router(state))`.
5. **Migrations** — Add SQL migrations in `ferrum-core/migrations/` (or a dedicated crate if needed) for any new tables.
6. **Docs** — Update [ARCHITECTURE.md](docs/ARCHITECTURE.md), [GA4GH.md](docs/GA4GH.md), and [README.md](README.md) with the new service and endpoints.
7. **Tests** — Add unit tests and, if possible, an integration test that hits the new routes.

---

## Rust style guide

- **Formatting:** `rustfmt` (project default).
- **Linting:** `clippy` with `-D warnings`; fix all warnings in new code.
- **Naming:** Follow Rust API guidelines (e.g. `snake_case` for functions, `PascalCase` for types).
- **Errors:** Use `thiserror` / `anyhow` as in the rest of the repo; prefer typed errors in libraries.
- **Tests:** Add or extend **unit tests** for non-trivial logic (parsers, auth checks, URL/storage helpers). Prefer **deterministic** tests without network or real cloud credentials unless the crate already uses an integration pattern for them.
- **Docs:** Public items should have **`///` rustdoc** when behaviour is not obvious; link related types and security-relevant caveats where useful.

---

*[← Back to README](README.md)*
