#!/usr/bin/env sh
# Fast DRS /stream smoke: seeded object microbench-plain-v1 (4096 bytes, plaintext S3 path).
# Run against a running gateway (e.g. CI after compose up). Exits non-zero on mismatch.
set -e
BASE="${GATEWAY_BASE:-http://localhost:8080}"
URL="${BASE}/ga4gh/drs/v1/objects/microbench-plain-v1/stream"
EXPECTED_SHA256="26b7e40be0bcf3e6667020b3acf6e07faa17585b21b2936305dd6c9ad3860b15"
EXPECTED_BYTES=4096

code=$(curl -sS -o /tmp/ferrum-microbench-stream.out -w "%{http_code}" "$URL" || echo "000")
if [ "$code" != "200" ]; then
  echo "ci-drs-microbench-stream: expected HTTP 200, got $code for $URL" >&2
  echo "ci-drs-microbench-stream: stream response body (JSON error or empty if router 404):" >&2
  curl -sS "$URL" 2>/dev/null | head -c 1200 >&2 || true
  echo >&2
  META="${BASE}/ga4gh/drs/v1/objects/microbench-plain-v1"
  mc=$(curl -sS -o /dev/null -w "%{http_code}" "$META" || echo "000")
  echo "ci-drs-microbench-stream: GET $META -> HTTP $mc (body snippet):" >&2
  curl -sS "$META" 2>/dev/null | head -c 800 >&2 || true
  echo >&2
  exit 1
fi

hdr=$(curl -sS -D - -o /dev/null "$URL" | tr -d '\r' | grep -i '^x-ferrum-drs-stream-path:' | head -1 || true)
if ! echo "$hdr" | grep -qi plaintext; then
  echo "ci-drs-microbench-stream: missing X-Ferrum-DRS-Stream-Path: plaintext (got: $hdr)" >&2
  exit 1
fi

n=$(wc -c </tmp/ferrum-microbench-stream.out | tr -d ' ')
if [ "$n" != "$EXPECTED_BYTES" ]; then
  echo "ci-drs-microbench-stream: expected $EXPECTED_BYTES bytes, got $n" >&2
  exit 1
fi

echo "${EXPECTED_SHA256}  /tmp/ferrum-microbench-stream.out" | sha256sum -c - >/dev/null

echo "ci-drs-microbench-stream: OK (plaintext /stream, ${EXPECTED_BYTES} bytes, sha256 match)"
