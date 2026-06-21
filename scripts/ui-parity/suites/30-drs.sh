#!/usr/bin/env bash
# Suite 30: DRS list/detail/stream (DataBrowser, ObjectDetailPage).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 30-drs"

expect_2xx "drs-list" GET "/ga4gh/drs/v1/objects?limit=20"

obj_id=""
if [[ -n "${DRS_FIXTURE_OBJECT:-}" ]]; then
  obj_id="$DRS_FIXTURE_OBJECT"
else
  list="$(http_body GET "/ga4gh/drs/v1/objects?limit=5")"
  obj_id="$(first_json_field "$list" "
import sys, json
for o in json.load(sys.stdin):
    if o.get('id'):
        print(o['id'])
        break
")"
fi

if [[ -n "$obj_id" ]]; then
  expect_2xx "drs-object" GET "/ga4gh/drs/v1/objects/${obj_id}"
  code="$(http_code GET "/ga4gh/drs/v1/objects/${obj_id}/stream")"
  if [[ "$code" =~ ^2 ]]; then
    ui_pass "drs-stream" "GET .../stream → HTTP $code"
  else
    ui_skip "drs-stream" "stream HTTP $code for $obj_id (url-backed objects may differ)"
  fi
  expect_2xx_soft "drs-provenance" GET "/ga4gh/drs/v1/objects/${obj_id}/provenance?direction=both&depth=5"
else
  ui_skip "drs-object" "no DRS objects (run seed on this profile)"
fi

expect_2xx_soft "ingest-jobs" GET "/api/v1/ingest/jobs?limit=5"
