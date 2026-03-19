#!/usr/bin/env sh
# HelixTest E2E compares downloaded bytes to test-data/expected/e2e/result.txt.sha256.
# Upstream may ship placeholder "REPLACE_WITH_REAL_SHA256"; overwrite from the same URL Ferrum
# seeds for DRS object demo-sample-vcf (see deploy/scripts/init-demo.sh).
set -e
URL="${E2E_RESULT_CHECKSUM_URL:-https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md}"
OUT="${1:-helixtest-repo/helixtest/test-data/expected/e2e/result.txt.sha256}"
mkdir -p "$(dirname "$OUT")"
SHA=$(curl -fsSL "$URL" | sha256sum | awk '{print $1}')
printf '%s' "$SHA" > "$OUT"
echo "E2E expected SHA256 (from URL in init-demo.sh demo-sample-vcf) -> $SHA"
