#!/usr/bin/env bash
# Suite 50: WES + TRS (WorkflowCenter, StartAnalysisDialog, ToolRegistry).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 50-wes-trs"

expect_2xx "wes-runs-list" GET "/ga4gh/wes/v1/runs?page_size=20"
expect_2xx "trs-tools" GET "/ga4gh/trs/v2/tools"

desc="${TRS_DESCRIPTOR_PATH:-}"
if [[ -n "$desc" ]]; then
  expect_2xx "trs-descriptor" GET "$desc"
else
  ui_skip "trs-descriptor" "TRS_DESCRIPTOR_PATH not set for profile"
fi

if [[ -n "${WES_FIXTURE_RUN:-}" ]]; then
  expect_2xx_soft "wes-run-get" GET "/ga4gh/wes/v1/runs/${WES_FIXTURE_RUN}"
  expect_2xx_soft "wes-run-provenance" GET "/ga4gh/wes/v1/runs/${WES_FIXTURE_RUN}/provenance"
fi

from_date="$(date -u -v-30d +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d '30 days ago' +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)"
to_date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
expect_2xx_soft "wes-cost-summary" GET "/ga4gh/wes/v1/cost/summary?from_date=${from_date}&to_date=${to_date}"

if ui_tier_write && [[ -n "${WES_SUBMIT_WORKFLOW:-}" ]]; then
  wf_type="${WES_SUBMIT_TYPE:-WDL}"
  submit_payload="$(python3 - "$wf_type" "${WES_SUBMIT_WORKFLOW}" <<'PY'
import json, sys
print(json.dumps({
    "workflow_type": sys.argv[1],
    "workflow_type_version": "1.0",
    "workflow_url": sys.argv[2],
    "workflow_params": {},
    "tags": {"source": "ui-parity"},
}))
PY
)"
  submit_resp="$(http_body POST "/ga4gh/wes/v1/runs" -d "$submit_payload")"
  run_id="$(first_json_field "$submit_resp" "import sys,json; print(json.load(sys.stdin).get('run_id',''))")"
  if [[ -z "$run_id" ]]; then
    ui_fail "wes-submit" "POST /ga4gh/wes/v1/runs did not return run_id: ${submit_resp:0:200}"
  else
    ui_pass "wes-submit" "submitted run $run_id"
    final="$(poll_wes_run "$run_id" "COMPLETE" 90 || true)"
    if [[ "$final" == "COMPLETE" ]]; then
      ui_pass "wes-run-complete" "run $run_id → COMPLETE"
    elif [[ "${WES_REQUIRE_COMPLETE:-0}" == "1" ]]; then
      ui_fail "wes-run-complete" "run $run_id ended in $final"
    else
      ui_skip "wes-run-complete" "run $run_id ended in $final (noop/pilot tolerated)"
    fi
    expect_2xx_soft "wes-run-logs" GET "/ga4gh/wes/v1/runs/${run_id}/logs/stdout"
  fi
else
  ui_skip "wes-submit" "tier=read or WES_SUBMIT_WORKFLOW unset (use --tier write)"
fi
