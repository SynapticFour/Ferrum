#!/usr/bin/env bash
# Phase 6 E2E: backup round-trip, checksum verify, solar/battery power HTTP gate.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PROFILE="${CARGO_PROFILE:-release-edge}"
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
FERRUM_GATEWAY="${FERRUM_GATEWAY_BIN:-$TARGET_DIR/$PROFILE/ferrum-gateway}"
FERRUM_CLI="${FERRUM_CLI_BIN:-$TARGET_DIR/debug/ferrum}"

cargo build -p ferrum-cli >/dev/null
if [ ! -x "$FERRUM_GATEWAY" ]; then
  "$ROOT/scripts/build-edge-native.sh" --no-native-cpu --profile "$PROFILE"
fi
FERRUM_CLI="$TARGET_DIR/debug/ferrum"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ferrum-ops-e2e.XXXXXX")"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
POWER_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
BASE="http://127.0.0.1:${PORT}"
POWER_BASE="http://127.0.0.1:${POWER_PORT}"
FIXTURE="$ROOT/profiles/pipeline/fixtures/tiny.vcf"
BACKUP="$TMP/field-backup.tar.gz"
PID=""
POWER_PID=""

cleanup() {
  if [ -n "${PID:-}" ]; then kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; fi
  if [ -n "${POWER_PID:-}" ]; then kill "$POWER_PID" 2>/dev/null || true; wait "$POWER_PID" 2>/dev/null || true; fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/objects"
cat >"$TMP/config.toml" <<EOF
bind = "127.0.0.1:${PORT}"

# NON-PILOT field E2E — production default is require_auth=true
[auth]
require_auth = false

[africa]
offline_first = true
sqlite_path = "${TMP}/ferrum.db"
objects_path = "${TMP}/objects"

[database]
driver = "sqlite"
sqlite_path = "${TMP}/ferrum.db"

[storage]
backend = "local"
base_path = "${TMP}/objects"

[ops]
verify_checksums_on_startup = false
EOF

cat >"$TMP/power-config.toml" <<EOF
bind = "127.0.0.1:${POWER_PORT}"

# NON-PILOT field E2E — production default is require_auth=true
[auth]
require_auth = false

[africa]
offline_first = true
sqlite_path = "${TMP}/power.db"
objects_path = "${TMP}/power-objects"

[database]
driver = "sqlite"
sqlite_path = "${TMP}/power.db"

[storage]
backend = "local"
base_path = "${TMP}/power-objects"

[power]
enabled = true
EOF

mkdir -p "$TMP/power-objects"

echo "ci-field-ops-e2e: start edge gateway"
FERRUM_CONFIG="$TMP/config.toml" FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY" "$FERRUM_CLI" demo start --edge >/dev/null 2>&1 &
PID=$!
for _ in $(seq 1 40); do curl -sf "${BASE}/health" >/dev/null 2>&1 && break; sleep 0.25; done

echo "ci-field-ops-e2e: ingest fixture"
curl -sf -X POST "${BASE}/api/v1/ingest/upload" \
  -F "file=@${FIXTURE};filename=tiny.vcf" \
  -F "name=tiny.vcf" >/dev/null
sleep 3

export FERRUM_CONFIG="$TMP/config.toml"
"$FERRUM_CLI" backup verify

echo "ci-field-ops-e2e: backup create"
"$FERRUM_CLI" backup create --output "$BACKUP"

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""

echo "ci-field-ops-e2e: restore round-trip"
rm -f "${TMP}/ferrum.db"
rm -rf "${TMP}/objects"
mkdir -p "${TMP}/objects"
"$FERRUM_CLI" backup restore --archive "$BACKUP" --force

FERRUM_CONFIG="$TMP/config.toml" FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY" "$FERRUM_CLI" demo start --edge >/dev/null 2>&1 &
PID=$!
for _ in $(seq 1 40); do curl -sf "${BASE}/health" >/dev/null 2>&1 && break; sleep 0.25; done
"$FERRUM_CLI" backup verify

echo "ci-field-ops-e2e: emergency power mode rejects HTTP"
FERRUM_CONFIG="$TMP/power-config.toml" FERRUM_POWER_MODE=emergency \
  FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY" "$FERRUM_CLI" demo start --edge >/dev/null 2>&1 &
POWER_PID=$!
for _ in $(seq 1 40); do curl -sf "${POWER_BASE}/health" >/dev/null 2>&1 && break; sleep 0.25; done
CODE=$(curl -s -o /dev/null -w '%{http_code}' "${POWER_BASE}/health" || true)
if [ "$CODE" != "503" ]; then
  echo "expected HTTP 503 in emergency power mode, got ${CODE}" >&2
  exit 1
fi

echo "ci-field-ops-e2e: OK"
