#!/usr/bin/env bash
# Run pilot enrichment against a remote Ferrum gateway (e.g. Fly pasteur-pilot).
# Usage: BASE_URL=https://pasteur-pilot-ferrum.fly.dev ./scripts/seed-pilot-remote.sh
# Requires operator passport/JWT if the deployment enforces auth — set FERRUM_PASSPORT_JWT.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_URL="${BASE_URL:-${1:-}}"
if [[ -z "$BASE_URL" ]]; then
  echo "seed-pilot-remote: set BASE_URL (e.g. https://pasteur-pilot-ferrum.fly.dev)" >&2
  exit 1
fi

export BASE_URL="${BASE_URL%/}"
AUTH_HEADER=()
if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
  AUTH_HEADER=(-H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}")
fi

curl -sf "${AUTH_HEADER[@]}" "$BASE_URL/health" >/dev/null || {
  echo "seed-pilot-remote: gateway not reachable at $BASE_URL" >&2
  exit 1
}

echo "seed-pilot-remote: enriching $BASE_URL"
exec env BASE_URL="$BASE_URL" bash "$SCRIPT_DIR/seed-pilot-demo.sh"
