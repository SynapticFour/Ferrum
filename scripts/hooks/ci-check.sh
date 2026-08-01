#!/usr/bin/env bash
# Mirror primary CI cargo gates for Ferrum.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "ci-check: cargo fmt --check"
cargo fmt --all -- --check

echo "ci-check: cargo clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo "ci-check: tests"
cargo test --workspace

echo "ci-check: OK"
