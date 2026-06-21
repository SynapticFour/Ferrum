#!/usr/bin/env bash
# Suite 00: health + service-info (matches ServiceHealthPanel / useAdminConfig).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 00-health @ $BASE_URL"

expect_2xx "health-gateway" GET "/health"
expect_2xx "admin-config" GET "/admin/config"

if [[ "${REQUIRE_AUTH:-0}" == "1" ]] && [[ -z "${FERRUM_PASSPORT_JWT:-}" ]]; then
  ui_skip "health-drs-si" "authenticated service-info skipped (no JWT)"
else
  expect_2xx "health-drs-si" GET "/ga4gh/drs/v1/service-info"
  expect_2xx "health-wes-si" GET "/ga4gh/wes/v1/service-info"
  expect_2xx "health-trs-si" GET "/ga4gh/trs/v2/service-info"
  expect_2xx "health-beacon-si" GET "/ga4gh/beacon/v2/service-info"
fi

cfg="$(http_body GET "/admin/config")"
tes="$(first_json_field "$cfg" "import sys,json; print(json.load(sys.stdin).get('compute',{}).get('tes_backend',''))")"
if [[ -n "${TES_BACKEND:-}" ]] && [[ -n "$tes" ]] && [[ "$tes" != "${TES_BACKEND}" ]]; then
  ui_fail "admin-tes-backend" "expected tes_backend=${TES_BACKEND}, got ${tes}"
else
  ui_pass "admin-tes-backend" "tes_backend=${tes:-unknown}"
fi
