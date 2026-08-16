#!/usr/bin/env bash
# CI: start TES stack, assert CWL WES COMPLETE via test-tes + smoke-pilot.
# TinyGermlineHC (Cromwell) is not required COMPLETE on GitHub-hosted runners (nested Docker).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

cleanup() {
  docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.tes.yml down -v --remove-orphans 2>/dev/null || true
}
trap cleanup EXIT

export FERRUM_TES_DOCKER_PLATFORM="${FERRUM_TES_DOCKER_PLATFORM:-linux/amd64}"
export SMOKE_REQUIRE_COMPLETE=1
export DOCKER_BIN="${DOCKER_BIN:-$(command -v docker)}"

echo "ci-tes-pilot-e2e: make up-tes"
make up-tes

echo "ci-tes-pilot-e2e: make test-tes"
make test-tes

echo "ci-tes-pilot-e2e: make smoke-pilot (require COMPLETE)"
make smoke-pilot

if [ -n "${FERRUM_PASSPORT_JWT:-}" ]; then
  echo "ci-tes-pilot-e2e: optional authenticated ingest upload"
  auth_payload="$(mktemp)"
  printf 'pilot-auth-ingest-%s' "$(date +%s)" >"$auth_payload"
  auth_json="$(curl -sf -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" \
    -F "file=@${auth_payload};type=text/plain" \
    "http://127.0.0.1:8081/api/v1/ingest/upload")"
  printf '%s' "$auth_json" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d.get("status") in ("succeeded","running"), d'
  rm -f "$auth_payload"
  echo "ci-tes-pilot-e2e: authenticated ingest OK"
fi

echo "ci-tes-pilot-e2e: OK"
