#!/usr/bin/env bash
set -euo pipefail

BUNDLE_DIR="./offline-bundle"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundle-dir) BUNDLE_DIR="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

if [[ ! -d "${BUNDLE_DIR}" ]]; then
  echo "Bundle directory not found: ${BUNDLE_DIR}"
  exit 1
fi

(
  cd "${BUNDLE_DIR}"
  shasum -a 256 -c checksums.sha256
)

ARCHIVE="$(ls "${BUNDLE_DIR}"/images-*.tar.gz 2>/dev/null | head -n 1 || true)"
if [[ -z "${ARCHIVE}" ]]; then
  echo "No images archive found."
  exit 1
fi

echo "Loading images from ${ARCHIVE}..."
gunzip -c "${ARCHIVE}" | docker load
echo "Import complete."

