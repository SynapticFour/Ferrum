#!/usr/bin/env bash
# Local Ferrum + hosted ga4gh-infra / Keycloak AAI smoke tests.
# Prereq: hosted AAI running, make up-pilot-cloud (gateway :8080, UI :8082).
set -euo pipefail

GA4GH="${PILOT_CLOUD_GA4GH_URL:?set PILOT_CLOUD_GA4GH_URL to the hosted ga4gh-infra base URL}"
GATEWAY="${FERRUM_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
UI="${FERRUM_UI_URL:-http://localhost:${UI_PORT:-8082}}"

die() { echo "ci-pilot-cloud-e2e: $*" >&2; exit 1; }
ok() { echo "ci-pilot-cloud-e2e: OK — $*"; }

curl -sf "$GA4GH/service-info" >/dev/null || die "Hosted broker unreachable at $GA4GH"
ok "hosted broker service-info"

curl -sf "$GATEWAY/health" >/dev/null || die "local gateway /health failed"
ok "local gateway health"

cfg="$(curl -sf "$GATEWAY/admin/config")"
python3 - "$cfg" "$GA4GH" <<'PY' || die "admin config does not point at hosted broker"
import json, sys
c = json.loads(sys.argv[1])
fly = sys.argv[2].rstrip("/")
a = c.get("auth") or {}
assert a.get("require_auth") is True, a
assert a.get("mode") == "external", a
assert a.get("access_requests_enabled") is True, a
login = a.get("broker_login_url") or ""
assert login.startswith(fly), (login, fly)
assert "/login/keycloak" in login, login
d = c.get("discovery") or {}
assert d.get("enabled") is True, d
assert d.get("service_registry_url", "").startswith(fly), d
print("broker_login_url:", login)
print("discovery:", d.get("service_registry_url"))
PY
ok "gateway wired to hosted Keycloak + service registry"

access_status="$(curl -sf "$GATEWAY/access/v1/status" 2>/dev/null || echo '{}')"
python3 - "$access_status" <<'PY' || die "ADS proxy not available"
import json, sys
s = json.loads(sys.argv[1])
assert s.get("ads_available") is True, s
print("ads_base_url:", s.get("ads_base_url"))
PY
ok "ADS access proxy reachable"

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
