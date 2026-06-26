#!/usr/bin/env bash
# Clone ga4gh-infra at GA4GH_INFRA_REF (branch, tag, or commit SHA).
# Usage: GA4GH_INFRA_REF=... clone-ga4gh-infra.sh /path/to/dest
set -euo pipefail

DEST="${1:?destination path required}"
REF="${GA4GH_INFRA_REF:-main}"
REPO="${GA4GH_INFRA_REPO:-https://github.com/SynapticFour/ga4gh-infra.git}"

if [[ -d "$DEST/.git" ]]; then
  echo "clone-ga4gh-infra: already present at $DEST"
  exit 0
fi

if [[ "$REF" =~ ^(main|master)$ ]] || [[ "$REF" =~ ^v[0-9] ]] || [[ "$REF" =~ ^ga4gh-infra-v ]]; then
  git clone --depth 1 --branch "$REF" "$REPO" "$DEST"
else
  git clone --depth 1 "$REPO" "$DEST"
  git -C "$DEST" fetch --depth 1 origin "$REF"
  git -C "$DEST" checkout FETCH_HEAD
fi

echo "clone-ga4gh-infra: $DEST @ $(git -C "$DEST" rev-parse --short HEAD)"
