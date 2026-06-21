#!/usr/bin/env bash
# Ferrum ui-parity — API acceptance tests mirroring the web UI.
#
# Usage:
#   ./scripts/ui-parity/ui-parity.sh --profile fly [--tier read|write]
#   ./scripts/ui-parity/ui-parity.sh --profile up-tes [--tier write]
#   ./scripts/ui-parity/ui-parity.sh --profile up-pilot-cloud
#
# Fly profile: set FERRUM_PASSPORT_JWT (see pilot-deploy/scripts/obtain-passport.sh)
# and run pilot-deploy ./pilot.sh seed all first.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UI_PARITY_DIR="$ROOT/scripts/ui-parity"
# shellcheck source=lib/common.sh
source "$UI_PARITY_DIR/lib/common.sh"

PROFILE=""
UI_PARITY_TIER="read"
UI_PARITY_REPORT=""

usage() {
  cat <<EOF
Usage: $(basename "$0") --profile fly|up-tes|up-pilot-cloud [options]

Options:
  --profile NAME   Target deployment (required)
  --tier read|write   read = GET/service-info only; write = create/submit tests
  --report PATH    Write markdown report to PATH (also printed)
  -h, --help       Show this help

Environment:
  FERRUM_PASSPORT_JWT   Required for fly and up-pilot-cloud (authenticated routes)
  FERRUM_URL            Override base URL (fly profile)
  BASE_URL              Override gateway URL (local profiles)
  PILOT_DIR             Path to pilot-deploy (fly delegation to pilot-smoke.sh)

Make targets:
  make ui-parity-fly
  make ui-parity-tes
  make ui-parity-pilot-cloud
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --tier)
      UI_PARITY_TIER="${2:-read}"
      shift 2
      ;;
    --report)
      UI_PARITY_REPORT="${2:-}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$PROFILE" ]] || {
  usage >&2
  exit 2
}

PROFILE_FILE="$UI_PARITY_DIR/profiles/${PROFILE}.env"
[[ -f "$PROFILE_FILE" ]] || {
  echo "unknown profile: $PROFILE (expected $PROFILE_FILE)" >&2
  exit 2
}

# shellcheck source=/dev/null
source "$PROFILE_FILE"

# Fly: load pilot-deploy .env if present (FERRUM_URL, secrets paths).
if [[ "$PROFILE" == "fly" ]]; then
  pilot_dir="${PILOT_DIR:-$ROOT/../synapticfour-business/customers/pasteur-tunis/pilot-deploy}"
  if [[ -f "$pilot_dir/.env" ]]; then
    # shellcheck source=/dev/null
    source "$pilot_dir/.env"
    export FERRUM_URL="${FERRUM_URL:-https://${PILOT_PREFIX:-pasteur-pilot}-ferrum.fly.dev}"
    BASE_URL="$FERRUM_URL"
  fi
  if [[ -f "$pilot_dir/.pilot-secrets.env" ]] && [[ -z "${FERRUM_PASSPORT_JWT:-}" ]]; then
    # shellcheck source=/dev/null
    source "$pilot_dir/.pilot-secrets.env" 2>/dev/null || true
  fi
fi

ui_parity_init
ui_log "profile=$UI_PARITY_PROFILE tier=$UI_PARITY_TIER base=$BASE_URL"

preflight_fly() {
  curl -sf "${BASE_URL}/health" >/dev/null || {
    ui_fail "preflight-health" "gateway not reachable at $BASE_URL"
    ui_parity_exit
  }
  ui_pass "preflight-health" "gateway reachable"

  if [[ "${REQUIRE_AUTH:-0}" == "1" ]] && [[ -z "${FERRUM_PASSPORT_JWT:-}" ]]; then
    ui_fail "preflight-jwt" "FERRUM_PASSPORT_JWT required — run: pilot-deploy/scripts/obtain-passport.sh --write-env"
    ui_parity_exit
  fi

  if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
    code="$(http_code GET "/workspaces/v1/workspaces")"
    if [[ "$code" == "401" ]]; then
      ui_fail "preflight-jwt" "Passport rejected (HTTP 401) — obtain a fresh token"
      ui_parity_exit
    fi
    ui_pass "preflight-jwt" "Passport accepted (workspaces HTTP $code)"
  fi

  if [[ -f "${PILOT_DIR:-}/seed/.seed-state/state.json" ]]; then
    ui_pass "preflight-seed" "pilot seed state present"
  else
    ui_skip "preflight-seed" "no seed state — run: cd pilot-deploy && ./pilot.sh seed all"
  fi
}

preflight_local() {
  curl -sf "${BASE_URL}/health" >/dev/null || {
    ui_fail "preflight-health" "gateway not reachable at $BASE_URL (is the stack up?)"
    ui_parity_exit
  }
  ui_pass "preflight-health" "gateway reachable"
}

case "$PROFILE" in
  fly)
    preflight_fly
    if [[ "${DELEGATE_PILOT_SMOKE:-0}" == "1" ]]; then
      smoke="$PILOT_DIR/scripts/pilot-smoke.sh"
      if [[ -x "$smoke" ]]; then
        ui_log "delegating to pilot-smoke.sh"
        FERRUM_URL="$BASE_URL" bash "$smoke" || ui_fail "delegate-pilot-smoke" "pilot-smoke.sh failed"
      else
        ui_skip "delegate-pilot-smoke" "pilot-smoke.sh not found at $smoke"
      fi
    fi
    ;;
  up-tes | up-pilot-cloud)
    preflight_local
    if [[ "$PROFILE" == "up-pilot-cloud" ]] && [[ "${REQUIRE_AUTH:-0}" == "1" ]] && [[ -z "${FERRUM_PASSPORT_JWT:-}" ]]; then
      ui_fail "preflight-jwt" "FERRUM_PASSPORT_JWT required for up-pilot-cloud"
      ui_parity_exit
    fi
    if [[ "$PROFILE" == "up-tes" ]] && [[ "${DELEGATE_SMOKE_PILOT:-0}" == "1" ]] && ui_tier_write; then
      ui_log "delegating to smoke-pilot-local.sh (write tier)"
      BASE_URL="$BASE_URL" bash "$ROOT/scripts/smoke-pilot-local.sh" || ui_fail "delegate-smoke-pilot" "smoke-pilot-local.sh failed"
    fi
    ;;
  *)
    ui_fail "preflight" "unknown profile $PROFILE"
    ui_parity_exit
    ;;
esac

SUITES=(
  "$UI_PARITY_DIR/suites/00-health.sh"
  "$UI_PARITY_DIR/suites/10-auth.sh"
  "$UI_PARITY_DIR/suites/20-workspaces.sh"
  "$UI_PARITY_DIR/suites/30-drs.sh"
  "$UI_PARITY_DIR/suites/40-cohorts.sh"
  "$UI_PARITY_DIR/suites/50-wes-trs.sh"
  "$UI_PARITY_DIR/suites/60-beacon.sh"
  "$UI_PARITY_DIR/suites/70-access.sh"
  "$UI_PARITY_DIR/suites/80-admin.sh"
)

for suite in "${SUITES[@]}"; do
  ui_log "--- $(basename "$suite") ---"
  bash "$suite" || ui_fail "suite-$(basename "$suite" .sh)" "suite exited with error"
done

ui_parity_exit
