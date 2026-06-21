#!/usr/bin/env bash
# Shared helpers for ui-parity suites.
set -euo pipefail

if [[ -z "${UI_PARITY_LIB_LOADED:-}" ]]; then
  UI_PARITY_LIB_LOADED=1
  UI_PARITY_PASS=0
  UI_PARITY_FAIL=0
  UI_PARITY_SKIP=0
  UI_PARITY_REPORT_LINES=()
fi

ui_parity_init() {
  BASE_URL="${BASE_URL%/}"
  export BASE_URL
  export UI_PARITY_TIER="${UI_PARITY_TIER:-read}"
  export UI_PARITY_PROFILE="${UI_PARITY_PROFILE:-unknown}"
}

ui_log() {
  printf 'ui-parity: %s\n' "$*"
}

ui_pass() {
  local id="$1" msg="$2"
  UI_PARITY_PASS=$((UI_PARITY_PASS + 1))
  UI_PARITY_REPORT_LINES+=("PASS | $id | $msg")
  printf '[PASS] %s — %s\n' "$id" "$msg"
}

ui_fail() {
  local id="$1" msg="$2"
  UI_PARITY_FAIL=$((UI_PARITY_FAIL + 1))
  UI_PARITY_REPORT_LINES+=("FAIL | $id | $msg")
  printf '[FAIL] %s — %s\n' "$id" "$msg" >&2
}

ui_skip() {
  local id="$1" msg="$2"
  UI_PARITY_SKIP=$((UI_PARITY_SKIP + 1))
  UI_PARITY_REPORT_LINES+=("SKIP | $id | $msg")
  printf '[SKIP] %s — %s\n' "$id" "$msg"
}

ui_tier_write() {
  [[ "${UI_PARITY_TIER:-read}" == "write" ]]
}

# End a suite when sourced from ui-parity.sh, or exit when run standalone.
ui_suite_done() {
  local code="${1:-0}"
  if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    return "$code"
  fi
  exit "$code"
}

auth_curl() {
  if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
    curl -sS -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" "$@"
  else
    curl -sS "$@"
  fi
}

http_code() {
  local method="$1" path="$2"
  shift 2
  local url="${BASE_URL}${path}"
  auth_curl -o /dev/null -w '%{http_code}' -X "$method" \
    -H 'Content-Type: application/json' \
    "$@" \
    "$url" 2>/dev/null || echo "000"
}

http_body() {
  local method="$1" path="$2"
  shift 2
  local url="${BASE_URL}${path}"
  auth_curl -X "$method" \
    -H 'Content-Type: application/json' \
    "$@" \
    "$url" 2>/dev/null || true
}

expect_2xx() {
  local id="$1" method="$2" path="$3"
  shift 3
  local code
  code="$(http_code "$method" "$path" "$@")"
  if [[ "$code" =~ ^2[0-9][0-9]$ ]]; then
    ui_pass "$id" "$method $path → HTTP $code"
  else
    ui_fail "$id" "$method $path → HTTP $code (expected 2xx)"
  fi
}

expect_2xx_soft() {
  local id="$1" method="$2" path="$3"
  shift 3
  local code
  code="$(http_code "$method" "$path" "$@")"
  if [[ "$code" =~ ^2[0-9][0-9]$ ]]; then
    ui_pass "$id" "$method $path → HTTP $code"
  else
    ui_skip "$id" "$method $path → HTTP $code (optional on this profile)"
  fi
}

require_jwt_or_skip_suite() {
  if [[ "${REQUIRE_AUTH:-0}" != "1" ]]; then
    return 0
  fi
  if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
    return 0
  fi
  ui_skip "auth-jwt" "REQUIRE_AUTH=1 but FERRUM_PASSPORT_JWT unset — run obtain-passport.sh"
  return 1
}

poll_wes_run() {
  local run_id="$1" want_state="${2:-COMPLETE}" max="${3:-120}"
  local state=""
  local i
  for i in $(seq 1 "$max"); do
    state="$(http_body GET "/ga4gh/wes/v1/runs/${run_id}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('state',''))" 2>/dev/null || true)"
    case "$state" in
      "$want_state") printf '%s' "$state"; return 0 ;;
      EXECUTOR_ERROR|SYSTEM_ERROR|CANCELED)
        printf '%s' "$state"
        return 1
        ;;
    esac
    sleep 1
  done
  printf '%s' "${state:-TIMEOUT}"
  return 1
}

first_json_field() {
  local json="$1" py="$2"
  printf '%s' "$json" | python3 -c "$py" 2>/dev/null || true
}

ui_parity_report() {
  local report_path="${UI_PARITY_REPORT:-}"
  {
    echo "# Ferrum ui-parity report"
    echo ""
    echo "- Profile: ${UI_PARITY_PROFILE}"
    echo "- Tier: ${UI_PARITY_TIER}"
    echo "- Base URL: ${BASE_URL}"
    echo "- Time (UTC): $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo ""
    echo "## Summary"
    echo "- PASS: ${UI_PARITY_PASS}"
    echo "- FAIL: ${UI_PARITY_FAIL}"
    echo "- SKIP: ${UI_PARITY_SKIP}"
    echo ""
    echo "## Results"
    local line
    for line in "${UI_PARITY_REPORT_LINES[@]}"; do
      echo "- $line"
    done
  } | if [[ -n "$report_path" ]]; then
    tee "$report_path"
  else
    cat
  fi
}

ui_parity_exit() {
  ui_parity_report
  if [[ "$UI_PARITY_FAIL" -gt 0 ]]; then
    ui_log "FAILED ($UI_PARITY_FAIL failures)"
    exit 1
  fi
  ui_log "passed ($UI_PARITY_PASS checks, $UI_PARITY_SKIP skipped)"
  exit 0
}
