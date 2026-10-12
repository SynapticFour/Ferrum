#!/usr/bin/env bash
# Print extra `cargo build -p ferrum-gateway` flags for an image variant.
# Sourced by Dockerfiles and scripts/build-variant-image.sh.
#
# Env:
#   FERRUM_VARIANT            full | edge | edge-infra | full-infra  (default: full)
#   FERRUM_GATEWAY_FEATURES   if set, overrides variant (comma-separated cargo features;
#                             always passed with --no-default-features)
set -euo pipefail

FEATURES="${FERRUM_GATEWAY_FEATURES:-}"
VARIANT="${FERRUM_VARIANT:-full}"

if [[ -n "$FEATURES" ]]; then
  printf '%s\n' "--no-default-features --features ${FEATURES}"
  exit 0
fi

case "$VARIANT" in
  full | "")
    # Default cargo features (`full` on ferrum-gateway).
    printf '\n'
    ;;
  edge)
    printf '%s\n' "--no-default-features --features edge"
    ;;
  edge-infra)
    printf '%s\n' "--no-default-features --features edge,external-auth"
    ;;
  full-infra)
    # Full surfaces (WES/TES/TRS/admin/Postgres) plus ga4gh-infra external-auth.
    printf '%s\n' "--features external-auth"
    ;;
  *)
    echo "error: unknown FERRUM_VARIANT=${VARIANT} (expected full|edge|edge-infra|full-infra)" >&2
    exit 1
    ;;
esac
