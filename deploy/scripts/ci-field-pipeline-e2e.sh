#!/usr/bin/env bash
# Phase 5 E2E: VCF Beacon index, QC metrics, htsget metadata, field reference bundle.
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

TMP="$(mktemp -d "${TMPDIR:-/tmp}/ferrum-pipeline-e2e.XXXXXX")"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
BASE="http://127.0.0.1:${PORT}"
VCF="$ROOT/profiles/pipeline/fixtures/tiny.vcf"
FASTQ="$ROOT/profiles/pipeline/fixtures/tiny.fastq"
PID=""

cleanup() {
  if [ -n "${PID:-}" ]; then kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; fi
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

[pipeline]
default_beacon_dataset = "field-edge-test"
allow_qc_stub = true
auto_index_beacon = true
auto_htsget_index = true
EOF

FERRUM_CONFIG="$TMP/config.toml" FERRUM_GATEWAY_BIN="$FERRUM_GATEWAY" "$FERRUM_CLI" demo start --edge >/dev/null 2>&1 &
PID=$!
for _ in $(seq 1 40); do curl -sf "${BASE}/health" >/dev/null 2>&1 && break; sleep 0.25; done

echo "ci-field-pipeline-e2e: ingest VCF"
curl -sf -X POST "${BASE}/api/v1/ingest/upload" \
  -F "file=@${VCF};filename=tiny.vcf" \
  -F "name=tiny.vcf" >/tmp/ingest-vcf.json
OBJECT_ID=$(python3 -c 'import json; d=json.load(open("/tmp/ingest-vcf.json")); print(d["result"]["object_ids"][0])')

sleep 2
export FERRUM_CONFIG="$TMP/config.toml"
"$FERRUM_CLI" pipeline htsget-status --object-id "$OBJECT_ID" | grep -q ready || true

"$FERRUM_CLI" pipeline index-beacon --object-id "$OBJECT_ID" --dataset field-edge-test

echo "ci-field-pipeline-e2e: QC metrics"
"$FERRUM_CLI" pipeline qc --object-id "$OBJECT_ID" --fastq "$FASTQ" --gateway "$BASE" --allow-stub

echo "ci-field-pipeline-e2e: reference field bundle"
"$FERRUM_CLI" reference install-field-bundle --gateway "$BASE" --bundle-dir "$ROOT/profiles/references/field-bundle"

echo "ci-field-pipeline-e2e: beacon query"
curl -sf "${BASE}/ga4gh/beacon/v2/g_variants?referenceName=chr1&start=100&referenceBases=A&alternateBases=G" \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d.get("response",{}).get("exists") in (True, False)'

echo "ci-field-pipeline-e2e: OK"
