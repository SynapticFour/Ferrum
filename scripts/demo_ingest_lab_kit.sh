#!/usr/bin/env bash
# Demo: /api/v1/ingest register + upload against a running ferrum-gateway.
# Usage: BASE_URL=http://127.0.0.1:8080 ./scripts/demo_ingest_lab_kit.sh
set -euo pipefail
BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
BASE_URL="${BASE_URL%/}"

echo "== POST register (url) =="
REG_JSON=$(curl -sS -X POST "$BASE_URL/api/v1/ingest/register" \
  -H "Content-Type: application/json" \
  -d "{\"client_request_id\":\"lab-kit-demo-register\",\"items\":[{\"kind\":\"url\",\"url\":\"https://example.com/\",\"name\":\"demo-url\"}]}")
echo "$REG_JSON" | head -c 800
echo
OBJ=$(echo "$REG_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('result',{}).get('object_ids',[''])[0])" 2>/dev/null || true)
if [[ -n "$OBJ" ]]; then
  echo "== GET DRS object $OBJ =="
  curl -sS "$BASE_URL/ga4gh/drs/v1/objects/$OBJ" | head -c 600
  echo
fi

TMP=$(mktemp)
echo "lab-kit-demo-bytes" >"$TMP"
echo "== POST upload =="
UP_JSON=$(curl -sS -X POST "$BASE_URL/api/v1/ingest/upload" \
  -F "client_request_id=lab-kit-demo-upload" \
  -F "file=@$TMP;type=text/plain")
rm -f "$TMP"
echo "$UP_JSON" | head -c 800
echo
JOB=$(echo "$UP_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('job_id',''))" 2>/dev/null || true)
if [[ -n "$JOB" ]]; then
  echo "== GET job $JOB =="
  curl -sS "$BASE_URL/api/v1/ingest/jobs/$JOB" | head -c 800
  echo
fi
