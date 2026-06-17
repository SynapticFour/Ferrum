#!/usr/bin/env bash
# Local Docker demo + TES: DRS ingest/stream and WES lifecycle (Docker-backed TES).
# Prereq: make up-tes (or stack already running on :8080).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASE="${FERRUM_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
PAYLOAD="${TMPDIR:-/tmp}/ferrum-tes-e2e-$$.bin"
WORKFLOW="$(cat <<'JSON'
{"workflow_type":"CWL","workflow_type_version":"v1.0","workflow_url":"https://raw.githubusercontent.com/common-workflow-language/user_guide/main/metadata/test_cwl/hello.cwl","workflow_params":{"message":"tes-e2e"}}
JSON
)"

cleanup() { rm -f "$PAYLOAD"; }
trap cleanup EXIT

die() { echo "ci-docker-tes-e2e: $*" >&2; exit 1; }

printf 'TES E2E payload %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$PAYLOAD"

echo "ci-docker-tes-e2e: health @ $BASE"
curl -sf "$BASE/health" >/dev/null || die "health check failed"

echo "ci-docker-tes-e2e: DRS ingest"
ingest_json="$(curl -sf -F "file=@${PAYLOAD};type=application/octet-stream" \
  "$BASE/ga4gh/drs/v1/ingest/file")"
object_id="$(printf '%s' "$ingest_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
[[ -n "$object_id" ]] || die "missing object id: $ingest_json"

curl -sf "$BASE/ga4gh/drs/v1/objects/${object_id}/stream" -o "${PAYLOAD}.dl"
cmp -s "$PAYLOAD" "${PAYLOAD}.dl" || die "stream byte mismatch"

echo "ci-docker-tes-e2e: WES submit"
run_json="$(curl -sf -H 'Content-Type: application/json' -d "$WORKFLOW" \
  "$BASE/ga4gh/wes/v1/runs")"
run_id="$(printf '%s' "$run_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("run_id",""))')"
[[ -n "$run_id" ]] || die "missing run_id: $run_json"

state=""
for _ in $(seq 1 90); do
  state="$(curl -sf "$BASE/ga4gh/wes/v1/runs/${run_id}/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state",""))' 2>/dev/null || true)"
  case "$state" in
    COMPLETE|CANCELED|EXECUTOR_ERROR|SYSTEM_ERROR) break ;;
  esac
  sleep 2
done
[[ "$state" == "COMPLETE" ]] || die "run did not COMPLETE (state=$state)"

echo "ci-docker-tes-e2e: OK (ingest, stream, WES COMPLETE)"
