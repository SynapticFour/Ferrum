#!/usr/bin/env bash
# Local gate before a v* tag.
# Covers fmt, clippy, workspace tests, SPDX, and cargo-deny when that binary
# is installed. Docker TES, HelixTest conformance, ARM, UI parity, and CodeQL
# stay workflow_dispatch (see docs/CI.md).
set -euo pipefail
# shellcheck disable=SC1091
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/hooks/macos-sdk.sh"
use_linkable_macos_sdk
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "verify-release: cargo fmt --check"
cargo fmt --all -- --check

echo "verify-release: cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "verify-release: workspace tests"
cargo test --workspace --all-targets

echo "verify-release: SPDX headers"
python3 scripts/spdx-rs.py . --license BUSL-1.1 --check

if cargo deny --version >/dev/null 2>&1; then
  echo "verify-release: cargo deny"
  cargo deny check
else
  echo "verify-release: cargo-deny not installed; skipped (cargo install cargo-deny)"
fi

echo "verify-release: OK"
