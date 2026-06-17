#!/usr/bin/env bash
# Local Docker demo (make up): health, DRS ingest, stream round-trip.
set -euo pipefail

BASE="${FERRUM_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
PAYLOAD="${TMPDIR:-/tmp}/ferrum-demo-e2e-$$.bin"

cleanup() { rm -f "$PAYLOAD" "${PAYLOAD}.dl"; }
trap cleanup EXIT

die() { echo "ci-docker-demo-e2e: $*" >&2; exit 1; }

printf 'demo e2e %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$PAYLOAD"

curl -sf "$BASE/health" >/dev/null || die "health failed"
curl -sf "$BASE/ga4gh/drs/v1/service-info" >/dev/null || die "DRS service-info failed"

ingest_json="$(curl -sf -F "file=@${PAYLOAD};type=application/octet-stream" \
  "$BASE/ga4gh/drs/v1/ingest/file")"
object_id="$(printf '%s' "$ingest_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
[[ -n "$object_id" ]] || die "no object id"

curl -sf "$BASE/ga4gh/drs/v1/objects/${object_id}/stream" -o "${PAYLOAD}.dl"
cmp -s "$PAYLOAD" "${PAYLOAD}.dl" || die "stream mismatch"

echo "ci-docker-demo-e2e: OK"
