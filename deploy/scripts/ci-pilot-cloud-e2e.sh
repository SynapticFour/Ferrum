#!/usr/bin/env bash
# Local Ferrum + Fly ga4gh-infra / Keycloak AAI smoke tests.
# Prereq: Fly pilot running, make up-pilot-cloud (gateway :8080, UI :8082).
set -euo pipefail

GA4GH="${PILOT_CLOUD_GA4GH_URL:-https://pasteur-pilot-ga4gh-infra.fly.dev}"
GATEWAY="${FERRUM_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
UI="${FERRUM_UI_URL:-http://localhost:${UI_PORT:-8082}}"

die() { echo "ci-pilot-cloud-e2e: $*" >&2; exit 1; }
ok() { echo "ci-pilot-cloud-e2e: OK — $*"; }

curl -sf "$GA4GH/service-info" >/dev/null || die "Fly broker unreachable at $GA4GH (run ./pilot.sh resume all --wait)"
ok "Fly broker service-info"

curl -sf "$GATEWAY/health" >/dev/null || die "local gateway /health failed"
ok "local gateway health"

cfg="$(curl -sf "$GATEWAY/admin/config")"
python3 - "$cfg" "$GA4GH" <<'PY' || die "admin config does not point at Fly broker"
import json, sys
c = json.loads(sys.argv[1])
fly = sys.argv[2].rstrip("/")
a = c.get("auth") or {}
assert a.get("require_auth") is True, a
assert a.get("mode") == "external", a
login = a.get("broker_login_url") or ""
assert login.startswith(fly), (login, fly)
assert "/login/keycloak" in login, login
print("broker_login_url:", login)
PY
ok "gateway wired to Fly Keycloak login"

curl -sf -o /dev/null "$UI/" || die "UI not reachable"
ok "UI reachable"

if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
  code="$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" \
    "$GATEWAY/workspaces/v1/workspaces")"
  [[ "$code" =~ ^(200|401|403)$ ]] || die "workspaces HTTP $code (expected 200/401/403)"
  ok "workspaces API with Bearer (HTTP $code)"
else
  echo "ci-pilot-cloud-e2e: skip Bearer tests (sign in at $UI then export FERRUM_PASSPORT_JWT from sessionStorage)"
fi

echo "ci-pilot-cloud-e2e: all checks passed"
