#!/usr/bin/env bash
# Build an offline operator documentation bundle for Edge field labs.
#
# Usage: ./scripts/build-edge-doc-bundle.sh [--output ferrum-edge-docs.tar.gz]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OUTPUT="${1:-ferrum-edge-docs.tar.gz}"
if [[ "${1:-}" == "--output" ]]; then
  OUTPUT="${2:-ferrum-edge-docs.tar.gz}"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

copy() {
  local src="$1"
  local dest="$2"
  if [[ -f "$src" ]]; then
    mkdir -p "$(dirname "$TMP/$dest")"
    cp "$src" "$TMP/$dest"
  fi
}

copy docs/AFRICA-DEPLOYMENT.md docs/AFRICA-DEPLOYMENT.md
copy docs/internal/FIELD-MATURITY-PLAN.md docs/FIELD-MATURITY-PLAN.md
copy docs/FIELD-SYNC-QUEUE.md docs/FIELD-SYNC-QUEUE.md
copy docs/INSTALLATION.md docs/INSTALLATION.md
copy docs/INGEST-LAB-KIT.md docs/INGEST-LAB-KIT.md
copy docs/PERFORMANCE.md docs/PERFORMANCE.md
copy README.md README.md

cat >"$TMP/README-OFFLINE.txt" <<'EOF'
Ferrum Edge offline documentation bundle.

Open docs/AFRICA-DEPLOYMENT.md for field deployment steps.
See docs/FIELD-MATURITY-PLAN.md for the full roadmap.
EOF

tar -czf "$OUTPUT" -C "$TMP" .
echo "[ferrum] Wrote offline doc bundle: $OUTPUT"
