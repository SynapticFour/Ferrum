# Contributing to Ferrum

Thank you for your interest in contributing. This document covers our code of conduct, development setup, testing, PR process, adding a new GA4GH service, and Rust style.

---

## Who builds Ferrum

Ferrum is developed by a small team of individuals on the autism spectrum based in Germany 🇩🇪. We believe that the attention to detail, systematic thinking, and deep focus that characterizes our work makes us particularly well-suited to building infrastructure that researchers can rely on.

We welcome contributors from all backgrounds. We value clear communication, well-documented decisions, and thorough reviews over speed. If you have questions about our process or need accommodations to contribute comfortably, please reach out — we understand.

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
- **CI:** GitHub Actions runs `cargo test --all` and `cargo clippy --all-targets --all -- -D warnings` on push and PRs.

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
3. **State** — Define an `AppState` (or use shared state) with DB pool, storage, and any service-specific handles.
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

---

*[← Back to README](README.md)*
