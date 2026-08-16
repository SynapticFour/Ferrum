#!/usr/bin/env bash
# Pi-class MinION chain (GitHub-hosted): edge gateway under a memory cap →
# simulated POD5 stubs land in a watch folder → ferrum ingest watch → DRS ont_metrics
# → optional HelixTest ferrum-africa --africa-profile ont.
#
# Not a USB gadget, not MinKNOW, not a real Raspberry Pi.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PROFILE="${CARGO_PROFILE:-release}"
PORT="${GATEWAY_PORT:-8080}"
BASE="${GATEWAY_BASE:-http://127.0.0.1:${PORT}}"
POLL_SECS="${WATCH_POLL_SECS:-2}"
DROP_PAUSE_SECS="${DROP_PAUSE_SECS:-10}"
MAX_MEMORY_MB="${FERRUM_AFRICA_MAX_MEMORY_MB:-3072}"

# Do not treat a rust-cache default-features ferrum-gateway as ready: that binary
# misses africa/ont_metrics. Skip compile only when CI passes an explicit path.
if [ -n "${FERRUM_GATEWAY_BIN:-}" ]; then
  GW="$FERRUM_GATEWAY_BIN"
  test -x "$GW"
else
  echo "ci-minion-field-chain: $(date -u +%FT%TZ) building ferrum-gateway --features edge --profile ${PROFILE}" >&2
  cargo build -p ferrum-gateway --profile "$PROFILE" --features edge
  GW="$ROOT/target/${PROFILE}/ferrum-gateway"
fi
if [ -n "${FERRUM_CLI_BIN:-}" ]; then
  CLI="$FERRUM_CLI_BIN"
  test -x "$CLI"
else
  # Debug ingest client. LTO release of ferrum-cli + gateway is what burned the 45m job.
  echo "ci-minion-field-chain: $(date -u +%FT%TZ) building ferrum-cli (dev)" >&2
  cargo build -p ferrum-cli
  CLI="$ROOT/target/debug/ferrum"
fi
echo "ci-minion-field-chain: $(date -u +%FT%TZ) binaries ready gw=$GW cli=$CLI" >&2

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ferrum-minion-chain.XXXXXX")"
WATCH="$TMP/minion_run"
mkdir -p "$WATCH" "$TMP/objects"
GW_LOG="$TMP/gateway.log"
WATCH_LOG="$TMP/watch.log"
GW_PID=""
WATCH_PID=""

cleanup() {
  if [ -n "${WATCH_PID:-}" ]; then
    kill "$WATCH_PID" 2>/dev/null || true
    wait "$WATCH_PID" 2>/dev/null || true
  fi
  if [ -n "${GW_PID:-}" ]; then
    kill "$GW_PID" 2>/dev/null || true
    wait "$GW_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

cat >"$TMP/config.toml" <<EOF
bind = "127.0.0.1:${PORT}"

# NON-PILOT: unauthenticated field ingest. Production default is require_auth=true.
[auth]
require_auth = false

[africa]
offline_first = true
max_memory_mb = ${MAX_MEMORY_MB}
sqlite_path = "${TMP}/ferrum.db"
objects_path = "${TMP}/objects"
EOF

export FERRUM_CONFIG="$TMP/config.toml"
export FERRUM_OFFLINE=1
export FERRUM_DEMO=1
export FERRUM_AUTH__REQUIRE_AUTH=false
export FERRUM_AFRICA__OFFLINE_FIRST=true
export FERRUM_AFRICA__MAX_MEMORY_MB="$MAX_MEMORY_MB"

echo "ci-minion-field-chain: Pi-class edge on ${BASE} (max_memory_mb=${MAX_MEMORY_MB})"
"$GW" >>"$GW_LOG" 2>&1 &
GW_PID=$!

ready=0
for _ in $(seq 1 40); do
  if curl -sf "${BASE}/health" >/dev/null 2>&1; then
    ready=1
    break
  fi
  if ! kill -0 "$GW_PID" 2>/dev/null; then
    echo "ci-minion-field-chain: gateway exited before health (log):" >&2
    tail -80 "$GW_LOG" >&2 || true
    exit 1
  fi
  sleep 0.5
done
if [ "$ready" -ne 1 ]; then
  echo "ci-minion-field-chain: gateway not healthy" >&2
  tail -80 "$GW_LOG" >&2 || true
  exit 1
fi

code_si=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}/ga4gh/drs/v1/service-info" || echo "000")
if [ "$code_si" != "200" ]; then
  echo "ci-minion-field-chain: DRS service-info expected 200, got ${code_si}" >&2
  exit 1
fi

echo "ci-minion-field-chain: watching ${WATCH} (poll ${POLL_SECS}s)"
"$CLI" ingest watch "$WATCH" --gateway "$BASE" --poll-secs "$POLL_SECS" >>"$WATCH_LOG" 2>&1 &
WATCH_PID=$!

for _ in $(seq 1 20); do
  if grep -q "Watching" "$WATCH_LOG" 2>/dev/null; then
    break
  fi
  if ! kill -0 "$WATCH_PID" 2>/dev/null; then
    echo "ci-minion-field-chain: ingest watch exited (log):" >&2
    cat "$WATCH_LOG" >&2 || true
    exit 1
  fi
  sleep 0.25
done

drop_stub() {
  local sample="$1"
  python3 - "$WATCH/${sample}.pod5" "$sample" <<'PY'
import sys
path, sample = sys.argv[1], sys.argv[2]
data = b"POD5\0\x01\x00" + sample.encode("ascii") + b"\0STUB_ONT_DATA"
open(path, "wb").write(data)
print(f"ci-minion-field-chain: dropped {path}", flush=True)
PY
}

drop_stub "read-001"
sleep "$DROP_PAUSE_SECS"
drop_stub "read-002"
sleep "$DROP_PAUSE_SECS"
drop_stub "read-003"

# Wait for three ingest log lines (poll + POST).
ingested=0
for _ in $(seq 1 40); do
  ingested=$(grep -c "Ingested" "$WATCH_LOG" 2>/dev/null || true)
  if [ "${ingested:-0}" -ge 3 ]; then
    break
  fi
  if ! kill -0 "$WATCH_PID" 2>/dev/null; then
    echo "ci-minion-field-chain: ingest watch died; log:" >&2
    cat "$WATCH_LOG" >&2 || true
    exit 1
  fi
  sleep 1
done
if [ "${ingested:-0}" -lt 3 ]; then
  echo "ci-minion-field-chain: expected 3 ingested files, got ${ingested:-0}" >&2
  cat "$WATCH_LOG" >&2 || true
  exit 1
fi

objects_json="$(curl -sf "${BASE}/ga4gh/drs/v1/objects")"
python3 -c '
import json, sys
raw = sys.stdin.read()
data = json.loads(raw)
objs = data if isinstance(data, list) else data.get("objects") or data.get("drs_objects") or []
with_metrics = [o for o in objs if o.get("ont_metrics")]
if len(with_metrics) < 1:
    raise SystemExit("no DRS object with ont_metrics: " + raw[:800])
print(f"ci-minion-field-chain: {len(with_metrics)} object(s) with ont_metrics")
' <<<"$objects_json"

echo "ci-minion-field-chain: watch-folder ingest OK"

if [ -n "${HELIXTEST_BIN:-}" ] || [ -n "${HELIXTEST_DIR:-}" ]; then
  if [ -n "${HELIXTEST_BIN:-}" ]; then
    HT="$HELIXTEST_BIN"
    test -x "$HT"
  else
    echo "ci-minion-field-chain: $(date -u +%FT%TZ) building HelixTest release..." >&2
    (
      cd "$HELIXTEST_DIR"
      cargo build --locked --release --bin helixtest
    )
    HT="$HELIXTEST_DIR/target/release/helixtest"
  fi
  echo "ci-minion-field-chain: $(date -u +%FT%TZ) HelixTest ferrum-africa --africa-profile ont" >&2
  GATEWAY_BASE="$BASE" \
  DRS_URL="${BASE}/ga4gh/drs/v1" \
  BEACON_URL="${BASE}/ga4gh/beacon/v2" \
  WES_URL="${BASE}/ga4gh/wes/v1" \
  "$HT" \
    --all --mode ferrum-africa --africa-profile ont --fail-level 1
fi

echo "ci-minion-field-chain: OK (Pi-class, simulated reads, no USB/MinKNOW)"
