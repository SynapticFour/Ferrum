#!/usr/bin/env bash
# Compare Ferrum git SHA embedded in running gateways (local compose + optional Fly).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_SHA="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
FLY_URL="${FERRUM_URL:-https://pasteur-pilot-ferrum.fly.dev}"
LOCAL_URL="${LOCAL_GATEWAY_URL:-http://localhost:${GATEWAY_PORT:-8080}}"

fetch_build_sha() {
  local base="$1"
  curl -sf "${base%/}/admin/config" 2>/dev/null \
    | python3 -c "import json,sys; d=json.load(sys.stdin); print((d.get('build') or {}).get('git_sha',''))" 2>/dev/null \
    || echo ""
}

fail=0
check_one() {
  local label="$1" url="$2" expect="${3:-$LOCAL_SHA}"
  local remote profile
  remote="$(fetch_build_sha "$url")"
  profile="$(curl -sf "${url%/}/admin/config" 2>/dev/null \
    | python3 -c "import json,sys; d=json.load(sys.stdin); print((d.get('build') or {}).get('profile',''))" 2>/dev/null || true)"
  if [[ -z "$remote" || "$remote" == "unknown" ]]; then
    printf '[WARN] %s — no build.git_sha at %s/admin/config (rebuild with FERRUM_GIT_SHA)\n' "$label" "$url"
    fail=1
    return
  fi
  if [[ "$remote" == "$expect" ]]; then
    printf '[OK]   %s git_sha=%s profile=%s\n' "$label" "$remote" "${profile:-?}"
  else
    printf '[FAIL] %s git_sha=%s (expected %s) profile=%s\n' "$label" "$remote" "$expect" "${profile:-?}"
    fail=1
  fi
}

echo "=== Ferrum source parity ==="
echo "Local repo:  $LOCAL_SHA ($REPO_ROOT)"
echo ""

check_one "Fly pilot" "$FLY_URL" "$LOCAL_SHA"

if curl -sf "${LOCAL_URL%/}/health" >/dev/null 2>&1; then
  check_one "Local gateway" "$LOCAL_URL" "$LOCAL_SHA"
else
  echo "[SKIP] Local gateway not running at $LOCAL_URL (make up / up-tes / up-pilot)"
fi

echo ""
if [[ "$fail" -ne 0 ]]; then
  echo "parity: MISMATCH — rebuild/redeploy so FERRUM_BUILD__GIT_SHA matches git HEAD"
  echo "  make up-tes | make up-pilot | pilot-deploy ./pilot.sh deploy ferrum"
  exit 1
fi
echo "parity: OK"
