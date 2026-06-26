#!/usr/bin/env bash
# Clone HelixTest at HELIXTEST_REF (branch, tag, or commit SHA).
# Usage: HELIXTEST_REF=... clone-helixtest.sh /path/to/dest
set -euo pipefail

DEST="${1:?destination path required}"
REF="${HELIXTEST_REF:-main}"
REPO="${HELIXTEST_REPO:-https://github.com/SynapticFour/HelixTest.git}"

if [[ -d "$DEST/.git" ]]; then
  echo "clone-helixtest: already present at $DEST"
  exit 0
fi

if [[ "$REF" =~ ^(main|master)$ ]] || [[ "$REF" =~ ^v[0-9] ]]; then
  git clone --depth 1 --branch "$REF" "$REPO" "$DEST"
else
  git clone --depth 1 "$REPO" "$DEST"
  git -C "$DEST" fetch --depth 1 origin "$REF"
  git -C "$DEST" checkout FETCH_HEAD
fi

echo "clone-helixtest: $DEST @ $(git -C "$DEST" rev-parse --short HEAD)"
