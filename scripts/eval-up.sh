#!/usr/bin/env bash
# Auth-on clone path: Ferrum + HS256, no sibling ga4gh-infra, no "read pilot.toml".
# Overlay is deploy/docker-compose.pilot-auth-ci.yml (require_auth=true, stubs off).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
ENVF="$ROOT/.eval.env"

if [ -z "${FERRUM_AUTH__JWT_SECRET:-}" ]; then
  if [ -f "$ENVF" ]; then
    # shellcheck disable=SC1090
    set -a && . "$ENVF" && set +a
  else
    SECRET="$(openssl rand -hex 32)"
    printf 'FERRUM_AUTH__JWT_SECRET=%s\n' "$SECRET" >"$ENVF"
    FERRUM_AUTH__JWT_SECRET="$SECRET"
    echo "Wrote $ENVF (gitignored)."
  fi
fi
export FERRUM_AUTH__JWT_SECRET

COMPOSE=(docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.pilot-auth-ci.yml)
"${COMPOSE[@]}" up -d --build

echo "Waiting for gateway (max 60s)..."
ok=0
for i in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:${GATEWAY_PORT:-8080}/health" >/dev/null; then
    echo "Gateway OK"
    ok=1
    break
  fi
  sleep 2
done
if [ "$ok" != 1 ]; then
  echo "Gateway did not become healthy. Check: ${COMPOSE[*]} logs ferrum-gateway" >&2
  exit 1
fi

TOKEN="$(python3 deploy/scripts/mint-hs256-jwt.py)"
echo ""
echo "Auth-on eval stack is up (HS256, not ga4gh-infra Passports)."
echo "  Gateway: http://127.0.0.1:${GATEWAY_PORT:-8080}"
echo "  export TEST_BEARER=$TOKEN"
echo "  export HELIXTEST_SHARED_SECRET=$FERRUM_AUTH__JWT_SECRET"
echo "Demo (auth-off) remains: make up. Stop this stack: make down-eval."
