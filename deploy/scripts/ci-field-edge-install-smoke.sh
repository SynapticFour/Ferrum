#!/usr/bin/env bash
# CI smoke: install-field-edge.sh builds Ferrum Edge and checks /health (Phase 3.1).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

export BUILD_FROM_SOURCE=1
export SKIP_INFRA=1
export FERRUM_INSTALL_DIR="${FERRUM_INSTALL_DIR:-$ROOT/.ci-ferrum-bin}"

rm -rf "$FERRUM_INSTALL_DIR"
./scripts/install-field-edge.sh

export PATH="$FERRUM_INSTALL_DIR:$PATH"
command -v ferrum-gateway >/dev/null
command -v ferrum >/dev/null

PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
export FERRUM_BIND="127.0.0.1:${PORT}"
LOG="$(mktemp -t ferrum-field-install-smoke.XXXXXX.log)"

ferrum demo start --edge 2>"$LOG" &
PID=$!
cleanup() {
  kill "$PID" 2>/dev/null || true
  pkill -P "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 90); do
  if curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

BODY="$(curl -sf "http://127.0.0.1:${PORT}/health")"
echo "$BODY" | grep -q '"status"' || { echo "health missing status: $BODY"; exit 1; }
echo "$BODY" | grep -q '"clock"' || { echo "health missing clock: $BODY"; exit 1; }

echo "ci-field-edge-install-smoke: OK (install-field-edge.sh + /health + clock)"
