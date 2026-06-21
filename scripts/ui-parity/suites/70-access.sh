#!/usr/bin/env bash
# Suite 70: Access / ADS (AccessManagement, DatasetCatalogPanel).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 70-access"

status="$(http_body GET "/access/v1/status")"
ads_ok="$(first_json_field "$status" "
import sys, json
d = json.load(sys.stdin)
print('1' if d.get('available') or d.get('ads_available') else '0')
" 2>/dev/null || echo 0)"

if [[ "${ADS_EXPECTED:-0}" == "1" ]]; then
  if [[ "$ads_ok" == "1" ]]; then
    ui_pass "access-status" "ADS available"
  else
    ui_fail "access-status" "ADS expected but /access/v1/status not available"
  fi
else
  if [[ "$ads_ok" == "1" ]]; then
    ui_pass "access-status" "ADS available"
  else
    ui_skip "access-status" "ADS not configured on this profile"
    exit 0
  fi
fi

if [[ "${REQUIRE_AUTH:-0}" == "1" ]] && ! require_jwt_or_skip_suite; then
  exit 0
fi

expect_2xx_soft "access-catalog" GET "/access/v1/catalog/datasets?resource_type=dataset"
expect_2xx_soft "access-grants" GET "/access/v1/me/grants"
expect_2xx_soft "access-projects" GET "/access/v1/me/projects"
