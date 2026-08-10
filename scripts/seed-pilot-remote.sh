#!/usr/bin/env bash
# Verify pilot/demo data on a remote Ferrum gateway (e.g. a Fly deployment).
#
# Set FERRUM_PASSPORT_JWT and BASE_URL, then run:
#   FERRUM_PASSPORT_JWT=… BASE_URL=https://your-ferrum.example ./scripts/seed-pilot-remote.sh
#
# Local enrichment (Docker stack with MinIO fixtures):
#   make seed-pilot
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_URL="${BASE_URL:-${1:-}}"
if [[ -z "$BASE_URL" ]]; then
  echo "seed-pilot-remote: set BASE_URL (e.g. https://pasteur-pilot-ferrum.fly.dev)" >&2
  echo "  Fly seed: cd synapticfour-business/customers/pasteur-tunis/pilot-deploy && ./pilot.sh seed all" >&2
  exit 1
fi

export BASE_URL="${BASE_URL%/}"
export PILOT_WORKSPACE_ID="${PILOT_WORKSPACE_ID:-demo-workspace-01}"
export PILOT_COHORT_ID="${PILOT_COHORT_ID:-demo-cohort-01}"
export PILOT_SAMPLE_ID="${PILOT_SAMPLE_ID:-pilot-demo-01}"

curl_pilot() {
  if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
    curl -fsS -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" "$@"
  else
    curl -fsS "$@"
  fi
}

if [[ -z "${FERRUM_PASSPORT_JWT:-}" ]]; then
  echo "seed-pilot-remote: WARN — FERRUM_PASSPORT_JWT unset; Fly pilot usually requires auth" >&2
fi

curl_pilot "$BASE_URL/health" >/dev/null || {
  echo "seed-pilot-remote: gateway not reachable at $BASE_URL" >&2
  exit 1
}

echo "seed-pilot-remote: verifying $BASE_URL (workspace=${PILOT_WORKSPACE_ID}, cohort=${PILOT_COHORT_ID})"
echo "  remote deploys may use different DRS object names than local make seed-pilot."

echo "seed-pilot-remote: verify DRS has objects"
obj_count="$(curl_pilot "$BASE_URL/ga4gh/drs/v1/objects?limit=5" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")"
if [[ "$obj_count" -lt 1 ]]; then
  echo "seed-pilot-remote: FAIL — no DRS objects (run ./pilot.sh seed all on Fly, or make seed-pilot locally)" >&2
  exit 1
fi
echo "seed-pilot-remote: OK — DRS lists objects (count sample: $obj_count+)"

# Optional: verify local pilot fixture names when present (make seed-pilot)
if curl_pilot "$BASE_URL/ga4gh/drs/v1/objects?limit=500" | python3 -c "
import json, os, sys
need = {
    'Pilot demo VCF (MinIO)',
    'Pilot demo BAM (MinIO)',
}
objs = json.load(sys.stdin)
names = {o.get('name') for o in objs}
if need.issubset(names):
    print('local-pilot-fixtures')
    sys.exit(0)
sys.exit(1)
" 2>/dev/null; then
  echo "seed-pilot-remote: OK — local pilot fixture names present (make seed-pilot)"
fi

# Fly pilot fixture names (pilot-deploy seed manifest)
if curl_pilot "$BASE_URL/ga4gh/drs/v1/objects?limit=500" | python3 -c "
import json, sys
need = {
    'na12878_slice.bam',
    'truth_slice.vcf.gz',
    'ref_slice.fa',
}
objs = json.load(sys.stdin)
names = {o.get('name') for o in objs}
if need.issubset(names):
    print('fly-pilot-fixtures')
    sys.exit(0)
sys.exit(1)
" 2>/dev/null; then
  echo "seed-pilot-remote: OK — Fly pilot fixture names present (./pilot.sh seed all)"
fi

echo "seed-pilot-remote: verify cohort sample ${PILOT_SAMPLE_ID} (optional)"
if curl_pilot "$BASE_URL/cohorts/v1/cohorts/${PILOT_COHORT_ID}/samples?limit=20" 2>/dev/null | python3 -c "
import json, os, sys
sample = os.environ.get('PILOT_SAMPLE_ID', 'pilot-demo-01')
data = json.load(sys.stdin)
samples = data.get('samples') or []
ids = {s.get('sample_id') for s in samples}
if sample not in ids:
    sys.exit(1)
row = next(s for s in samples if s.get('sample_id') == sample)
drs = row.get('drs_object_ids') or []
if len(drs) < 1:
    sys.exit(2)
print(f'OK — cohort sample {sample} with {len(drs)} DRS object(s)')
" 2>/dev/null; then
  :
else
  echo "seed-pilot-remote: WARN — cohort ${PILOT_COHORT_ID} / sample ${PILOT_SAMPLE_ID} not found (seed may use different IDs on Fly)" >&2
fi

echo "seed-pilot-remote: verification passed @ $BASE_URL"
echo "  UI: ${BASE_URL%/}/ui/"
