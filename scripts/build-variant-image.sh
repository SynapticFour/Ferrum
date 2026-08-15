#!/usr/bin/env bash
# Build a Ferrum gateway container image for a named variant and/or target platform.
#
# Usage:
#   ./scripts/build-variant-image.sh --variant edge
#   ./scripts/build-variant-image.sh --variant edge --platform linux/arm64
#   ./scripts/build-variant-image.sh --features edge,external-auth --tag ferrum:custom
#   ./scripts/build-variant-image.sh --dry-run --variant edge-infra
#
# Default Dockerfile is deploy/Dockerfile (musl/distroless, matches GHCR).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VARIANT="full"
FEATURES=""
PLATFORM=""
TAG=""
DOCKERFILE="deploy/Dockerfile"
DRY_RUN=0
GIT_SHA="${FERRUM_GIT_SHA:-unknown}"

usage() {
  cat <<'EOF'
Usage: ./scripts/build-variant-image.sh [options]

  --variant full|edge|edge-infra   Named Ferrum image variant (default: full)
  --features <cargo-features>      Override variant (comma-separated, --no-default-features)
  --platform linux/amd64|linux/arm64
  --tag <name:tag>                 Image tag (default: ferrum:<variant>)
  --file <Dockerfile>              Default: deploy/Dockerfile
  --dry-run                        Print docker build command and cargo flags; do not build

See docs/IMAGE-VARIANTS.md.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --variant)
      VARIANT="$2"
      shift 2
      ;;
    --features)
      FEATURES="$2"
      shift 2
      ;;
    --platform)
      PLATFORM="$2"
      shift 2
      ;;
    --tag)
      TAG="$2"
      shift 2
      ;;
    --file)
      DOCKERFILE="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$TAG" ]]; then
  TAG="ferrum:${VARIANT}"
fi

if [[ "$GIT_SHA" == "unknown" ]] && command -v git >/dev/null 2>&1; then
  GIT_SHA="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
fi

export FERRUM_VARIANT="$VARIANT"
export FERRUM_GATEWAY_FEATURES="$FEATURES"
FLAGS="$(bash "$ROOT/deploy/scripts/gateway-build-flags.sh")"
FLAGS="$(printf '%s' "$FLAGS" | tr '\n' ' ' | sed 's/[[:space:]]*$//')"

CMD=(docker build -f "$DOCKERFILE"
  --build-arg "FERRUM_VARIANT=${VARIANT}"
  --build-arg "FERRUM_GATEWAY_FEATURES=${FEATURES}"
  --build-arg "FERRUM_GIT_SHA=${GIT_SHA}"
  --build-arg "FERRUM_BUILD_PROFILE=${VARIANT}"
  -t "$TAG"
)

if [[ -n "$PLATFORM" ]]; then
  CMD+=(--platform "$PLATFORM")
fi

CMD+=("$ROOT")

echo "variant:  ${VARIANT}"
echo "features: ${FEATURES:-<(variant default)>}"
echo "cargo:    cargo build --release -p ferrum-gateway ${FLAGS}"
echo "tag:      ${TAG}"
[[ -n "$PLATFORM" ]] && echo "platform: ${PLATFORM}"

if [[ "$DRY_RUN" -eq 1 ]]; then
  printf 'dry-run:'
  printf ' %q' "${CMD[@]}"
  printf '\n'
  exit 0
fi

exec "${CMD[@]}"
