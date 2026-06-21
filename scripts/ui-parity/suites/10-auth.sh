#!/usr/bin/env bash
# Suite 10: auth wiring (admin/config — matches useAuthConfig).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 10-auth"

cfg="$(http_body GET "/admin/config")"
if python3 - "$cfg" "${REQUIRE_AUTH:-0}" <<'PY'
import json, sys
c = json.loads(sys.argv[1])
require = sys.argv[2] == "1"
a = c.get("auth") or {}
mode = a.get("mode")
if require:
    assert mode == "external", f"mode={mode}"
    assert a.get("require_auth") is True, a
    login = a.get("broker_login_url") or ""
    assert "login/" in login, login
else:
    assert a.get("require_auth") in (False, None), a
    assert mode in ("demo", "external", "builtin", None), mode
PY
then
  ui_pass "auth-config" "admin/config auth fields OK"
else
  ui_fail "auth-config" "admin/config auth fields invalid"
fi

if [[ -n "${GA4GH_URL:-}" ]]; then
  code="$(curl -sS -o /dev/null -w '%{http_code}' "${GA4GH_URL}/service-info" 2>/dev/null || echo 000)"
  if [[ "$code" =~ ^2 ]]; then
    ui_pass "auth-broker-si" "ga4gh broker service-info HTTP $code"
  else
    ui_fail "auth-broker-si" "ga4gh broker unreachable HTTP $code"
  fi
fi
