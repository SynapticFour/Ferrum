#!/usr/bin/env bash
# Run HelixTest against the Ferrum demo Docker stack (same flow as CI conformance.yml).
#
# Usage:
#   ./deploy/scripts/run-helixtest-local.sh              # core services (fast)
#   ./deploy/scripts/run-helixtest-local.sh --full       # full suite + microbench
#   ./deploy/scripts/run-helixtest-local.sh --no-docker  # stack already running
#   ./deploy/scripts/run-helixtest-local.sh --only htsget
#
# Env: HELIXTEST_REF (default main), GATEWAY_BASE (default http://localhost:8080)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Default HelixTest ref from VERSIONS.lock (override with HELIXTEST_REF=...)
if [[ -f "$ROOT/scripts/load-versions.sh" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT/scripts/load-versions.sh"
fi
HELIXTEST_REF="${HELIXTEST_REF:-main}"
GATEWAY_BASE="${GATEWAY_BASE:-http://localhost:8080}"
HELIXTEST_DIR="${HELIXTEST_DIR:-$ROOT/.helixtest-checkout}"
COMPOSE_FILE="$ROOT/deploy/docker-compose.yml"
MODE="core"
START_DOCKER=1
EXTRA_HELIX_ARGS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --full)
      MODE="full"
      shift
      ;;
    --no-docker)
      START_DOCKER=0
      shift
      ;;
    --only)
      EXTRA_HELIX_ARGS+=(--only "$2")
      shift 2
      ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

wait_for_gateway() {
  echo "Waiting for gateway ${GATEWAY_BASE}/health (up to 120s)..."
  for i in $(seq 1 24); do
    if curl -sf "${GATEWAY_BASE}/health" >/dev/null; then
      echo "Gateway ready."
      return 0
    fi
    sleep 5
  done
  echo "Gateway did not become healthy." >&2
  docker compose -f "$COMPOSE_FILE" logs ferrum-gateway 2>&1 | tail -80 || true
  return 1
}

if [ "$START_DOCKER" -eq 1 ]; then
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker not found; start the stack manually or install Docker." >&2
    exit 1
  fi
  chmod +x "$ROOT/deploy/scripts/docker-compose-build-retry.sh"
  "$ROOT/deploy/scripts/docker-compose-build-retry.sh" -f "$COMPOSE_FILE" build ferrum-init ferrum-gateway
  docker compose -f "$COMPOSE_FILE" up -d --build
  wait_for_gateway
fi

if [ ! -d "$HELIXTEST_DIR/.git" ]; then
  echo "Cloning HelixTest (${HELIXTEST_REF}) into ${HELIXTEST_DIR}..."
  HELIXTEST_REF="$HELIXTEST_REF" bash "$ROOT/deploy/scripts/clone-helixtest.sh" "$HELIXTEST_DIR"
else
  echo "Using existing HelixTest checkout at ${HELIXTEST_DIR}"
fi

SHA="$(sha256sum "$HELIXTEST_DIR/helixtest/test-data/workflows/outputs/tes_echo_out.txt" | awk '{print $1}')"
printf '%s' "$SHA" >"$HELIXTEST_DIR/helixtest/test-data/expected/workflows/tes_echo_out.txt.sha256"
echo "Aligned TES expected SHA256 -> $SHA"

HELIXTEST_DIR="$HELIXTEST_DIR" sh "$ROOT/deploy/scripts/align-helixtest-e2e-checksum.sh" \
  "$HELIXTEST_DIR/helixtest/test-data/expected/e2e/result.txt.sha256"

export GATEWAY_BASE
export FERRUM_PUBLIC_BASE_URL="${FERRUM_PUBLIC_BASE_URL:-$GATEWAY_BASE}"
export HELIXTEST_SKIP_AUTH="${HELIXTEST_SKIP_AUTH:-true}"
export WES_URL="${WES_URL:-$GATEWAY_BASE/ga4gh/wes/v1}"
export TES_URL="${TES_URL:-$GATEWAY_BASE/ga4gh/tes/v1}"
export DRS_URL="${DRS_URL:-$GATEWAY_BASE/ga4gh/drs/v1}"
export TRS_URL="${TRS_URL:-$GATEWAY_BASE/ga4gh/trs/v2}"
export BEACON_URL="${BEACON_URL:-$GATEWAY_BASE/ga4gh/beacon/v2}"
export AUTH_URL="${AUTH_URL:-$GATEWAY_BASE/passports/v1}"

echo "Running DRS microbench..."
GATEWAY_BASE="$GATEWAY_BASE" sh "$ROOT/deploy/scripts/ci-drs-microbench-stream.sh"

cd "$HELIXTEST_DIR"

case "$MODE" in
  full)
    echo "Running HelixTest full suite (--all --fail-level 1)..."
    cargo run --bin helixtest --release -- --all --mode ferrum --report json --fail-level 1
    ;;
  core)
    if [ "${#EXTRA_HELIX_ARGS[@]}" -gt 0 ]; then
      echo "Running HelixTest with extra filters: ${EXTRA_HELIX_ARGS[*]}"
      cargo run --bin helixtest --release -- --all --mode ferrum --report table --fail-level 2 "${EXTRA_HELIX_ARGS[@]}"
    else
      echo "Running HelixTest core services (WES, TES, DRS, TRS, Beacon)..."
      cargo run --bin helixtest --release -- --all --mode ferrum --only wes --only tes --only drs --only trs --only beacon --report table --fail-level 2
      echo "Running HelixTest htsget..."
      cargo run --bin helixtest --release -- --all --mode ferrum --only htsget --report table --fail-level 2
    fi
    ;;
esac

echo "HelixTest run finished successfully."
