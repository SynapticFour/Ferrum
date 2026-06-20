#!/usr/bin/env bash
# Run pilot enrichment against a remote Ferrum gateway (e.g. Fly pasteur-pilot).
# Usage: FERRUM_PASSPORT_JWT=… BASE_URL=https://pasteur-pilot-ferrum.fly.dev ./scripts/seed-pilot-remote.sh
# Requires operator passport/JWT when the deployment enforces auth.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_URL="${BASE_URL:-${1:-}}"
if [[ -z "$BASE_URL" ]]; then
  echo "seed-pilot-remote: set BASE_URL (e.g. https://pasteur-pilot-ferrum.fly.dev)" >&2
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

echo "seed-pilot-remote: enriching $BASE_URL (workspace=${PILOT_WORKSPACE_ID}, cohort=${PILOT_COHORT_ID})"
bash "$SCRIPT_DIR/seed-pilot-demo.sh"

echo "seed-pilot-remote: verify DRS pilot objects"
curl_pilot "$BASE_URL/ga4gh/drs/v1/objects" | python3 -c "
import json, os, sys

need = {
    'Pilot demo VCF (MinIO)',
    'Pilot demo BAM (MinIO)',
    'Pilot demo BAM index (MinIO)',
    'Pilot reference FASTA (MinIO)',
    'Pilot reference FASTA index (MinIO)',
    'Pilot truth VCF (MinIO)',
    'Pilot truth VCF index (MinIO)',
}
objs = json.load(sys.stdin)
names = {o.get('name') for o in objs}
missing = sorted(need - names)
if missing:
    print('seed-pilot-remote: FAIL — missing pilot objects:', ', '.join(missing), file=sys.stderr)
    sys.exit(1)
print(f'seed-pilot-remote: OK — {len(need)} pilot objects present')
"

echo "seed-pilot-remote: verify cohort sample ${PILOT_SAMPLE_ID}"
curl_pilot "$BASE_URL/cohorts/v1/cohorts/${PILOT_COHORT_ID}/samples?limit=20" | python3 -c "
import json, os, sys

sample = os.environ.get('PILOT_SAMPLE_ID', 'pilot-demo-01')
data = json.load(sys.stdin)
samples = data.get('samples') or []
ids = {s.get('sample_id') for s in samples}
if sample not in ids:
    print(f'seed-pilot-remote: FAIL — cohort sample {sample} not found (have: {sorted(ids)})', file=sys.stderr)
    sys.exit(1)
row = next(s for s in samples if s.get('sample_id') == sample)
drs = row.get('drs_object_ids') or []
if len(drs) < 2:
    print(f'seed-pilot-remote: FAIL — {sample} has {len(drs)} DRS objects, expected ≥2', file=sys.stderr)
    sys.exit(1)
print(f'seed-pilot-remote: OK — cohort sample {sample} with {len(drs)} DRS objects')
"

echo "seed-pilot-remote: all checks passed @ $BASE_URL"
echo "  UI: ${BASE_URL%/}/ui/"
echo "  Next: open Cohorts → ${PILOT_COHORT_ID} → Run on cohort (refs auto-fill from workspace objects)"
