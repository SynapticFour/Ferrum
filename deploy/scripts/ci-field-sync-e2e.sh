#!/usr/bin/env bash
# Phase 4 E2E: sync enqueue/status + push to a second Edge hub + beacon federation smoke.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PROFILE="${CARGO_PROFILE:-release-edge}"
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
HOST_TRIPLE="$(rustc -vV 2>/dev/null | awk '/host:/ {print $2}')"
FERRUM_GATEWAY="${FERRUM_GATEWAY_BIN:-$TARGET_DIR/$PROFILE/ferrum-gateway}"
FERRUM_CLI="${FERRUM_CLI_BIN:-$TARGET_DIR/debug/ferrum}"

if [ ! -x "$FERRUM_CLI" ]; then
  cargo build -p ferrum-cli
fi
# Always rebuild CLI in CI so sync subcommands match current tree.
cargo build -p ferrum-cli >/dev/null
FERRUM_CLI="$TARGET_DIR/debug/ferrum"
if [ ! -x "$FERRUM_GATEWAY" ]; then
  "$ROOT/scripts/build-edge-native.sh" --no-native-cpu --profile "$PROFILE"
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ferrum-sync-e2e.XXXXXX")"
EDGE_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
HUB_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
PEER_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
EDGE_BASE="http://127.0.0.1:${EDGE_PORT}"
HUB_BASE="http://127.0.0.1:${HUB_PORT}"
PEER_BASE="http://127.0.0.1:${PEER_PORT}"
PAYLOAD="$TMP/payload.bin"
EDGE_PID="" HUB_PID="" PEER_PID=""

cleanup() {
  for pid in "$EDGE_PID" "$HUB_PID" "$PEER_PID"; do
    if [ -n "${pid:-}" ]; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/edge/objects" "$TMP/hub/objects" "$TMP/peer/objects"
printf 'field sync E2E payload' >"$PAYLOAD"

write_config() {
  local name=$1 port=$2 db=$3 objects=$4 peer_url=${5:-}
  cat >"$TMP/${name}.toml" <<EOF
bind = "127.0.0.1:${port}"

[africa]
offline_first = true
sqlite_path = "${TMP}/${db}"
objects_path = "${objects}"

[sync]
default_target_url = "${HUB_BASE}"

[database]
driver = "sqlite"
sqlite_path = "${TMP}/${db}"
run_migrations = true

[storage]
backend = "local"
base_path = "${objects}"
EOF
  if [ -n "$peer_url" ]; then
    cat >>"$TMP/${name}.toml" <<EOF

[federation]
enabled = true
aggregate_strategy = "union"

[[federation.peers]]
name = "peer-edge"
beacon_endpoint = "${peer_url}/ga4gh/beacon/v2"
timeout_ms = 3000
EOF
  fi
}

write_config edge "$EDGE_PORT" edge.db "$TMP/edge/objects" "$PEER_BASE"
write_config hub "$HUB_PORT" hub.db "$TMP/hub/objects"
write_config peer "$PEER_PORT" peer.db "$TMP/peer/objects"

start_gateway() {
  local cfg=$1
  FERRUM_CONFIG="$cfg" FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY" "$FERRUM_CLI" demo start --edge >/dev/null 2>&1 &
  echo $!
}

wait_health() {
  local base=$1
  for _ in $(seq 1 40); do
    if curl -sf "${base}/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "ci-field-sync-e2e: timeout waiting for ${base}/health" >&2
  return 1
}

EDGE_PID="$(start_gateway "$TMP/edge.toml")"
HUB_PID="$(start_gateway "$TMP/hub.toml")"
PEER_PID="$(start_gateway "$TMP/peer.toml")"

wait_health "$EDGE_BASE"
wait_health "$HUB_BASE"
wait_health "$PEER_BASE"

echo "ci-field-sync-e2e: ingest on edge node"
curl -sf -X POST "${EDGE_BASE}/api/v1/ingest/upload" \
  -F "file=@${PAYLOAD};filename=sync-test.bin" \
  -F "name=sync-test.bin" >/dev/null

export FERRUM_CONFIG="$TMP/edge.toml"
"$FERRUM_CLI" sync enqueue --all-local --target "$HUB_BASE"
"$FERRUM_CLI" sync status | grep -q pending
"$FERRUM_CLI" sync push --target "$HUB_BASE" --dry-run
"$FERRUM_CLI" sync push --target "$HUB_BASE"
"$FERRUM_CLI" sync status | grep -q completed

EXPORT="$TMP/sneakernet.tar.gz"
"$FERRUM_CLI" sync export --output "$EXPORT"
test -s "$EXPORT"

echo "ci-field-sync-e2e: beacon federation smoke (edge → peer)"
# federate=true should not error when peer is unreachable or returns empty
curl -sf "${EDGE_BASE}/ga4gh/beacon/v2/g_variants?federate=true&referenceName=1&start=1&referenceBases=A&alternateBases=T" \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); assert "response" in d'

echo "ci-field-sync-e2e: OK (enqueue, push, export, federation query)"
