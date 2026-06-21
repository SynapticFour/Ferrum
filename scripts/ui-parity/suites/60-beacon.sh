#!/usr/bin/env bash
# Suite 60: Beacon (BeaconExplorer).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 60-beacon"

assembly="${BEACON_ASSEMBLY:-GRCh37}"
chr="${BEACON_CHR:-22}"
start="${BEACON_START:-2000}"
ref="${BEACON_REF:-T}"
alt="${BEACON_ALT:-G}"

payload="$(python3 - "$assembly" "$chr" "$start" "$ref" "$alt" <<'PY'
import json, sys
asm, chrom, start, ref, alt = sys.argv[1:6]
print(json.dumps({
    "meta": {"apiVersion": "v2.0.0"},
    "query": {
        "requestParameters": {
            "assemblyId": asm,
            "referenceName": chrom,
            "start": int(start),
            "referenceBases": ref,
            "alternateBases": alt,
            "requestedGranularity": "boolean",
        }
    },
}))
PY
)"

code="$(http_code POST "/ga4gh/beacon/v2/g_variants/query" -d "$payload")"
if [[ ! "$code" =~ ^2 ]]; then
  ui_fail "beacon-query" "POST g_variants/query → HTTP $code"
  exit 0
fi

resp="$(http_body POST "/ga4gh/beacon/v2/g_variants/query" -d "$payload")"
exists="$(first_json_field "$resp" "import sys,json; print(json.load(sys.stdin).get('response',{}).get('exists',''))")"
if [[ "$exists" == "True" ]] || [[ "$exists" == "true" ]]; then
  ui_pass "beacon-query" "chr${chr}:${start} ${ref}>${alt} exists=true"
else
  ui_skip "beacon-query" "query OK but exists=$exists (run seed on Fly)"
fi
