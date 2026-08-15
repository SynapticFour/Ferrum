#!/usr/bin/env bash
# Copy LinkML YAML from ferrum-meta into profiles/meta/schema/ (compile-time
# input of ferrum-meta-connect). --check diffs vendored copies against the
# sibling checkout and does not write files.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC="$ROOT/profiles/meta/sync-spec.json"
OUT="$ROOT/profiles/meta/schema"
CHECK=0
if [[ "${1:-}" == "--check" ]]; then
  CHECK=1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

VERSION=$(jq -r '.ferrum_meta_version' "$SPEC")
PINNED_SHA=$(jq -r '.ferrum_meta_sha' "$SPEC")

resolve_repo() {
  if [[ -n "${FERRUM_META_REPO:-}" && -d "${FERRUM_META_REPO}/schema/core" ]]; then
    printf '%s\n' "$FERRUM_META_REPO"
  elif [[ -d "$ROOT/../ferrum-meta/schema/core" ]]; then
    printf '%s\n' "$ROOT/../ferrum-meta"
  else
    printf ''
  fi
}

REPO="$(resolve_repo)"
if [[ -z "$REPO" ]]; then
  echo "[sync-ferrum-meta] ferrum-meta checkout not found."
  echo "[sync-ferrum-meta] Set FERRUM_META_REPO or clone next to Ferrum."
  if [[ "$CHECK" -eq 1 ]]; then
    exit 1
  fi
  echo "[sync-ferrum-meta] Leaving vendored schema in $OUT (version $VERSION)."
  exit 0
fi

echo "[sync-ferrum-meta] source: $REPO"
echo "[sync-ferrum-meta] ferrum-meta version: $VERSION (pin $PINNED_SHA)"

HEAD="$(git -C "$REPO" rev-parse HEAD)"
if [[ "$CHECK" -eq 1 && "$HEAD" != "$PINNED_SHA" ]]; then
  echo "[sync-ferrum-meta] warning: checkout HEAD $HEAD != pin $PINNED_SHA (diffing files anyway)"
fi

fail=0
while IFS= read -r row; do
  src_rel=$(jq -r '.schema_path' <<<"$row")
  dst_rel=$(jq -r '.vendored' <<<"$row")
  src="$REPO/$src_rel"
  dst="$ROOT/$dst_rel"
  if [[ ! -f "$src" ]]; then
    echo "missing source $src" >&2
    fail=1
    continue
  fi
  if [[ "$CHECK" -eq 1 ]]; then
    if ! diff -u "$dst" "$src"; then
      echo "DRIFT: $dst_rel != $src_rel" >&2
      fail=1
    fi
  else
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    echo "copied $src_rel -> $dst_rel"
  fi
done < <(jq -c '.profiles[]' "$SPEC")

if [[ "$CHECK" -eq 1 ]]; then
  if [[ "$fail" -ne 0 ]]; then
    echo "[sync-ferrum-meta] schema drift; run scripts/sync-ferrum-meta-schemas.sh" >&2
    exit 1
  fi
  echo "[sync-ferrum-meta] vendored YAML matches $REPO"
  exit 0
fi

{
  echo "Vendored from https://github.com/SynapticFour/ferrum-meta"
  echo "SHA: $HEAD"
  echo "Do not edit these YAML files by hand."
  echo "Refresh: scripts/sync-ferrum-meta-schemas.sh"
  echo "Check:   scripts/sync-ferrum-meta-schemas.sh --check"
} >"$OUT/SOURCE.txt"

if [[ "$HEAD" != "$PINNED_SHA" ]]; then
  echo "[sync-ferrum-meta] update ferrum_meta_sha in $SPEC and VERSIONS.lock to $HEAD"
fi

echo "[sync-ferrum-meta] Done. YAML is include_str! input of ferrum-meta-connect."
echo "[sync-ferrum-meta] Fixture validation: cargo test -p ferrum-meta-connect"
