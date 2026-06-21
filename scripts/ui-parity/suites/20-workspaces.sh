#!/usr/bin/env bash
# Suite 20: workspaces (WorkspaceListPage, WorkspaceDetailPage).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 20-workspaces"

if [[ "${REQUIRE_AUTH:-0}" == "1" ]] && ! require_jwt_or_skip_suite; then
  ui_suite_done 0
fi

expect_2xx "workspaces-list" GET "/workspaces/v1/workspaces"

list_body="$(http_body GET "/workspaces/v1/workspaces")"
ws_id="$(first_json_field "$list_body" "
import sys, json
d = json.load(sys.stdin)
if isinstance(d, list) and d:
    print(d[0].get('id',''))
")"

if [[ -n "$ws_id" ]]; then
  expect_2xx "workspaces-get" GET "/workspaces/v1/workspaces/${ws_id}"
  expect_2xx "workspaces-contents" GET "/workspaces/v1/workspaces/${ws_id}/contents"
  expect_2xx "workspaces-activity" GET "/workspaces/v1/workspaces/${ws_id}/activity"
  expect_2xx "workspaces-members" GET "/workspaces/v1/workspaces/${ws_id}/members"
else
  ui_skip "workspaces-detail" "no workspace yet (create one in UI or use --tier write)"
fi

if ui_tier_write; then
  slug="ui-parity-$$"
  create_body="$(http_body POST "/workspaces/v1/workspaces" -d "{\"name\":\"UI Parity $$\",\"description\":\"automated ui-parity\",\"slug\":\"${slug}\"}")"
  new_id="$(first_json_field "$create_body" "import sys,json; print(json.load(sys.stdin).get('id',''))")"
  if [[ -n "$new_id" ]]; then
    ui_pass "workspaces-create" "created workspace $new_id"
    expect_2xx "workspaces-get-new" GET "/workspaces/v1/workspaces/${new_id}"
  else
    ui_fail "workspaces-create" "POST /workspaces/v1/workspaces did not return id"
  fi
else
  ui_skip "workspaces-create" "tier=read (use --tier write to test create)"
fi
