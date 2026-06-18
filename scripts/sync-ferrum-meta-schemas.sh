#!/usr/bin/env bash
# Sync JSON Schema artifacts from ferrum-meta releases (Phase 2.4).
# Requires: curl, jq, optional ferrum-meta checkout at FERRUM_META_REPO.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC="$ROOT/profiles/meta/sync-spec.json"
OUT="$ROOT/profiles/meta/schemas"
mkdir -p "$OUT"

echo "[sync-ferrum-meta] Reading $SPEC"
VERSION=$(jq -r '.ferrum_meta_version' "$SPEC")
echo "[sync-ferrum-meta] ferrum-meta version: $VERSION"

if [[ -n "${FERRUM_META_REPO:-}" && -d "$FERRUM_META_REPO/schemas" ]]; then
  echo "[sync-ferrum-meta] Copying schemas from local checkout: $FERRUM_META_REPO"
  cp -v "$FERRUM_META_REPO"/schemas/*.yaml "$OUT/" 2>/dev/null || true
  cp -v "$FERRUM_META_REPO"/schemas/*.json "$OUT/" 2>/dev/null || true
else
  echo "[sync-ferrum-meta] No local ferrum-meta checkout (set FERRUM_META_REPO)."
  echo "[sync-ferrum-meta] CI validates structural rules via ferrum-meta-connect; full LinkML parity stays in ferrum-meta Python."
fi

echo "[sync-ferrum-meta] Validate fixtures:"
for fixture in "$ROOT"/profiles/meta/fixtures/*.yaml; do
  cargo run -q -p ferrum-cli -- meta validate --input "$fixture" || exit 1
done
echo "[sync-ferrum-meta] Done."
