#!/usr/bin/env bash
# Local make up-pilot stack: broker + Ferrum external auth smoke tests.
# Prereq: make up-pilot (broker :8180, gateway :8080, UI :8082).
set -euo pipefail

BROKER="${PILOT_BROKER_URL:-http://localhost:8180}"
GATEWAY="${FERRUM_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
UI="${FERRUM_UI_URL:-http://localhost:${UI_PORT:-8082}}"

die() { echo "ci-pilot-aai-e2e: $*" >&2; exit 1; }
ok() { echo "ci-pilot-aai-e2e: OK — $*"; }

curl -sf "$BROKER/service-info" >/dev/null || die "broker service-info failed"
ok "broker service-info"

cfg="$(curl -sf "$GATEWAY/admin/config")"
python3 - "$cfg" <<'PY' || die "admin config auth fields missing"
import json, sys
c = json.loads(sys.argv[1])
a = c.get("auth") or {}
assert a.get("require_auth") is True, a
assert a.get("mode") == "external", a
assert a.get("broker_login_url"), a
print("auth config:", a.get("broker_login_url"))
PY
ok "gateway external auth config"

curl -sf -o /dev/null "$UI/" || die "UI not reachable"
ok "UI reachable"

# Optional: authenticated API when passport provided (obtain via mock-idp login in browser).
if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
  code="$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" \
    "$GATEWAY/ga4gh/drs/v1/service-info")"
  [[ "$code" == "200" ]] || die "authenticated DRS service-info HTTP $code"
  ok "Bearer DRS service-info"
else
  echo "ci-pilot-aai-e2e: skip Bearer tests (set FERRUM_PASSPORT_JWT after mock-idp login)"
fi

echo "ci-pilot-aai-e2e: all checks passed"
