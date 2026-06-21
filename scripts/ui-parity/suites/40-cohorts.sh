#!/usr/bin/env bash
# Suite 40: cohorts (CohortListPage, CohortDetailPage).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 40-cohorts"

if [[ "${REQUIRE_AUTH:-0}" == "1" ]] && ! require_jwt_or_skip_suite; then
  ui_suite_done 0
fi

expect_2xx "cohorts-list" GET "/cohorts/v1/cohorts?limit=20"
expect_2xx_soft "cohorts-phenotype-schema" GET "/cohorts/v1/phenotype-schema"

cohort_id="${COHORT_ID:-}"
if [[ -z "$cohort_id" ]]; then
  list="$(http_body GET "/cohorts/v1/cohorts?limit=5")"
  cohort_id="$(first_json_field "$list" "
import sys, json
d = json.load(sys.stdin)
cohorts = d.get('cohorts') if isinstance(d, dict) else d
if isinstance(cohorts, list) and cohorts:
    print(cohorts[0].get('id',''))
")"
fi

if [[ -n "$cohort_id" ]]; then
  expect_2xx "cohorts-get" GET "/cohorts/v1/cohorts/${cohort_id}"
  expect_2xx "cohorts-samples" GET "/cohorts/v1/cohorts/${cohort_id}/samples?limit=50"
  expect_2xx_soft "cohorts-stats" GET "/cohorts/v1/cohorts/${cohort_id}/stats"
else
  ui_skip "cohorts-detail" "no cohort on this deployment"
fi
