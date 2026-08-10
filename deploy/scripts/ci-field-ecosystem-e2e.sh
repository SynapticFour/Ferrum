#!/usr/bin/env bash
# Phase 7 E2E: ecosystem docs, Africa HTTP gap tests (WES/bandwidth/power), i18n smoke.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "ci-field-ecosystem-e2e: required docs"
for f in \
  docs/FIELD-ECOSYSTEM.md \
  docs/FIELD-GA4GH-DEMO-PI.md \
  docs/FIELD-ECOSYSTEM.md; do
  test -f "$f"
done
test -x scripts/install-field-edge.sh

echo "ci-field-ecosystem-e2e: Africa HTTP gap tests (HelixTest supplements)"
cargo test -p ferrum-reference --quiet
cargo test -p ferrum-reference --test wes_mismatch --quiet
cargo test -p ferrum-storage --test bandwidth --quiet
cargo test -p ferrum-gateway --test power_limit --quiet
cargo test -p ferrum-core --test africa_network --quiet

echo "ci-field-ecosystem-e2e: CLI i18n field strings"
cargo test -p ferrum-cli i18n::tests::field_edge_strings --quiet

echo "ci-field-ecosystem-e2e: deprecated laptop wrapper still delegates"
grep -q ci-edge-demo-e2e.sh deploy/scripts/ci-laptop-demo-e2e.sh

echo "ci-field-ecosystem-e2e: OK"
