#!/usr/bin/env bash
# CI: start TES stack, assert WES COMPLETE via test-tes + smoke-pilot.
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

echo "ci-tes-pilot-e2e: OK"
