#!/usr/bin/env bash
# End-to-end smoke test: `ferrum demo start --offline` (SQLite + local DRS ingest/stream).
# Requires: curl, python3, cargo-built ferrum-cli + ferrum-gateway (or set FERRUM_*_BIN).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PROFILE="${CARGO_PROFILE:-debug}"
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
FERRUM_CLI="${FERRUM_CLI_BIN:-$TARGET_DIR/$PROFILE/ferrum}"
FERRUM_GATEWAY="${FERRUM_GATEWAY_BIN:-$TARGET_DIR/$PROFILE/ferrum-gateway}"

if [ ! -x "$FERRUM_CLI" ] || [ ! -x "$FERRUM_GATEWAY" ]; then
  echo "ci-laptop-demo-e2e: building ferrum-cli and ferrum-gateway ($PROFILE)..." >&2
  cargo build -p ferrum-cli -p ferrum-gateway
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ferrum-laptop-e2e.XXXXXX")"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
BASE="http://127.0.0.1:${PORT}"
LOG="$TMP/demo.log"
PAYLOAD="$TMP/payload.bin"
CLI_PID=""

cleanup() {
  if [ -n "${CLI_PID:-}" ]; then
    for pid in $(pgrep -P "$CLI_PID" 2>/dev/null || true); do
      kill "$pid" 2>/dev/null || true
    done
    kill "$CLI_PID" 2>/dev/null || true
    wait "$CLI_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/objects"
printf '%s' 'GA4GH laptop demo offline E2E payload' >"$PAYLOAD"

cat >"$TMP/config.toml" <<EOF
bind = "127.0.0.1:${PORT}"

[africa]
offline_first = true
sqlite_path = "${TMP}/ferrum.db"
objects_path = "${TMP}/objects"
EOF

export FERRUM_CONFIG="$TMP/config.toml"
export FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY"

echo "ci-laptop-demo-e2e: starting ferrum demo start --offline on ${BASE} (log: ${LOG})"
"$FERRUM_CLI" demo start --offline >>"$LOG" 2>&1 &
CLI_PID=$!

ready=0
for i in $(seq 1 40); do
  if curl -sf "${BASE}/health" >/dev/null 2>&1; then
    ready=1
    break
  fi
  if ! kill -0 "$CLI_PID" 2>/dev/null; then
    echo "ci-laptop-demo-e2e: ferrum demo exited before health check (see log):" >&2
    tail -80 "$LOG" >&2 || true
    exit 1
  fi
  sleep 0.25
done

if [ "$ready" -ne 1 ]; then
  echo "ci-laptop-demo-e2e: gateway not healthy after 10s" >&2
  tail -80 "$LOG" >&2 || true
  exit 1
fi

echo "ci-laptop-demo-e2e: health OK"

code_si=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}/ga4gh/drs/v1/service-info" || echo "000")
if [ "$code_si" != "200" ]; then
  echo "ci-laptop-demo-e2e: DRS service-info expected 200, got ${code_si}" >&2
  exit 1
fi

ingest_json="$(curl -sf -F "file=@${PAYLOAD};type=application/octet-stream" \
  "${BASE}/ga4gh/drs/v1/ingest/file")"
object_id="$(printf '%s' "$ingest_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
if [ -z "$object_id" ]; then
  echo "ci-laptop-demo-e2e: missing object id in ingest response: ${ingest_json}" >&2
  exit 1
fi

meta_code=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}/ga4gh/drs/v1/objects/${object_id}" || echo "000")
if [ "$meta_code" != "200" ]; then
  echo "ci-laptop-demo-e2e: GET object expected 200, got ${meta_code}" >&2
  exit 1
fi

curl -sf "${BASE}/ga4gh/drs/v1/objects/${object_id}/stream" -o "$TMP/downloaded.bin"
if ! cmp -s "$PAYLOAD" "$TMP/downloaded.bin"; then
  echo "ci-laptop-demo-e2e: stream bytes mismatch" >&2
  exit 1
fi

echo "ci-laptop-demo-e2e: OK (demo start --offline, ingest, metadata, stream round-trip)"
