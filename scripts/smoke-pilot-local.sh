#!/usr/bin/env bash
# Local pilot smoke test — run after make up-tes (optionally make seed-pilot).
# Verifies: health, lineage, managed-storage preview, cohort sample, WES/TES submit.
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
BASE_URL="${BASE_URL%/}"
RUN_SEED="${SMOKE_RUN_SEED:-1}"  # set 0 to skip make seed-pilot

die() { echo "smoke-pilot-local: FAIL — $*" >&2; exit 1; }
ok() { echo "smoke-pilot-local: OK — $*"; }

echo "smoke-pilot-local: health @ $BASE_URL"
curl -sf "$BASE_URL/health" >/dev/null || die "gateway not reachable (run: make up-tes)"

if [[ "$RUN_SEED" == "1" ]]; then
  echo "smoke-pilot-local: seed-pilot"
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  bash "$SCRIPT_DIR/seed-pilot-demo.sh"
fi

echo "smoke-pilot-local: lineage (microbench-plain-v1)"
prov="$(curl -fsS "$BASE_URL/ga4gh/drs/v1/objects/microbench-plain-v1/provenance?direction=both" || true)"
node_count="$(printf '%s' "$prov" | python3 -c "
import sys,json
d=json.load(sys.stdin)
g=d.get('graph') or d
print(len(g.get('nodes',[])))
" 2>/dev/null || echo 0)"
[[ "$node_count" -ge 1 ]] || die "lineage returned no nodes for microbench-plain-v1: $prov"
ok "lineage nodes=$node_count"

echo "smoke-pilot-local: lineage (demo-run-seed-01)"
run_prov="$(curl -fsS "$BASE_URL/ga4gh/wes/v1/runs/demo-run-seed-01/provenance" || true)"
run_nodes="$(printf '%s' "$run_prov" | python3 -c "
import sys,json
d=json.load(sys.stdin)
g=d.get('graph') or d
print(len(g.get('nodes',[])))
" 2>/dev/null || echo 0)"
[[ "$run_nodes" -ge 1 ]] || die "run lineage empty for demo-run-seed-01: $run_prov"
ok "run lineage nodes=$run_nodes"

echo "smoke-pilot-local: pilot VCF stream preview"
vcf_id="$(curl -fsS "$BASE_URL/ga4gh/drs/v1/objects" | python3 -c "
import sys, json
objs = json.load(sys.stdin)
for o in objs:
    if o.get('name') == 'Pilot demo VCF (MinIO)':
        print(o['id'])
        break
" 2>/dev/null || true)"
[[ -n "$vcf_id" ]] || die "Pilot demo VCF not found — seed-pilot may have failed"
preview="$(curl -fsS "$BASE_URL/ga4gh/drs/v1/objects/${vcf_id}/stream" | head -c 200 || true)"
printf '%s' "$preview" | grep -q 'fileformat=VCF' || die "VCF stream preview missing ##fileformat"
ok "VCF stream preview ($vcf_id)"

echo "smoke-pilot-local: cohort sample pilot-demo-01"
samples="$(curl -fsS "$BASE_URL/cohorts/v1/cohorts/demo-cohort-01/samples?limit=50")"
sample_count="$(printf '%s' "$samples" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('samples',[])))" 2>/dev/null || echo 0)"
drs_count="$(printf '%s' "$samples" | python3 -c "
import sys,json
for s in json.load(sys.stdin).get('samples',[]):
    if s.get('sample_id')=='pilot-demo-01':
        print(len(s.get('drs_object_ids',[])))
        break
" 2>/dev/null || echo 0)"
[[ "$sample_count" -ge 1 ]] || die "demo-cohort-01 has no samples"
[[ "$drs_count" -ge 3 ]] || die "pilot-demo-01 should link BAM+BAI+VCF (got $drs_count DRS ids)"
ok "cohort samples=$sample_count pilot-demo-01 drs=$drs_count"

echo "smoke-pilot-local: germline cohort wiring (TinyGermlineHC inputs)"
printf '%s' "$samples" | python3 -c "
import json, sys, urllib.request

samples = json.load(sys.stdin).get('samples', [])
pilot = next((s for s in samples if s.get('sample_id') == 'pilot-demo-01'), None)
if not pilot:
    raise SystemExit('pilot-demo-01 missing')
ids = pilot.get('drs_object_ids') or []
if len(ids) < 3:
    raise SystemExit(f'expected 3+ DRS ids, got {len(ids)}')

base = '${BASE_URL}'
objs = json.load(urllib.request.urlopen(f'{base}/ga4gh/drs/v1/objects'))
by_id = {o['id']: o for o in objs}

def kind(o):
    backend = o.get('storage_backend')
    if backend == 'url':
        return 'url'
    if backend in ('s3', 'local'):
        return 'managed'
    am = (o.get('access_methods') or [{}])[0]
    url = (am.get('access_url') or {}).get('url', '')
    if '/ga4gh/drs/v1/objects/' in url and '/access/' in url:
        return 'managed'
    desc = (o.get('description') or '').lower()
    name = (o.get('name') or '').lower()
    if 'url pointer' in desc or 'readme' in name or 'external alignment' in name:
        return 'url'
    if any(x in url for x in ('1000genomes', 'raw.githubusercontent.com', 'ftp.')):
        return 'url'
    return 'managed'

bam = bai = None
for oid in ids:
    o = by_id.get(oid)
    if not o:
        raise SystemExit(f'unknown object {oid}')
    if kind(o) == 'url':
        raise SystemExit(f'URL-backed object not valid for germline: {oid} ({o.get(\"name\")})')
    name = (o.get('name') or '').lower()
    mime = (o.get('mime_type') or '').lower()
    if 'bam' in mime or name.endswith('.bam') or 'bam (' in name:
        bam = oid
    if 'bai' in name or name.endswith('.bai') or 'index' in name:
        bai = oid

if not bam:
    raise SystemExit('no BAM object in pilot sample')
if not bai:
    raise SystemExit('no BAI object in pilot sample')
print(f'bam={bam} bai={bai}')
" || die "germline cohort wiring failed"
ok "germline BAM+BAI resolved for pilot-demo-01"

echo "smoke-pilot-local: reference bundle (TinyGermlineHC ref + truth)"
ref_count="$(curl -fsS "$BASE_URL/ga4gh/drs/v1/objects" | python3 -c "
import sys, json
need = {
    'Pilot reference FASTA (MinIO)',
    'Pilot reference FASTA index (MinIO)',
    'Pilot truth VCF (MinIO)',
    'Pilot truth VCF index (MinIO)',
}
found = {o.get('name') for o in json.load(sys.stdin)}
print(len(need & found))
" 2>/dev/null || echo 0)"
[[ "$ref_count" -ge 4 ]] || die "expected 4 pilot reference objects (got $ref_count) — run make seed-pilot"
ok "reference bundle objects=$ref_count"

echo "smoke-pilot-local: WES submit (TES lifecycle)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_CWL="${REPO_ROOT}/profiles/pipeline/fixtures/smoke-hello.cwl"
TES_BASE="${TES_GATEWAY_URL:-http://host.docker.internal:8080}"

if [[ -z "${SMOKE_CWL_URL:-}" && -f "$FIXTURE_CWL" ]]; then
  cwl_resp="$(curl -fsS -X POST "$BASE_URL/api/v1/ingest/upload" \
    -F "file=@${FIXTURE_CWL}" \
    -F "name=smoke-hello.cwl" \
    -F "client_request_id=smoke-pilot-cwl-v1")"
  cwl_job="$(printf '%s' "$cwl_resp" | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")"
  for _ in $(seq 1 30); do
    st="$(curl -fsS "$BASE_URL/api/v1/ingest/jobs/${cwl_job}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || true)"
    [[ "$st" == "succeeded" ]] && break
    [[ "$st" == "failed" ]] && die "smoke CWL ingest failed"
    sleep 1
  done
  cwl_id="$(curl -fsS "$BASE_URL/api/v1/ingest/jobs/${cwl_job}" | python3 -c "import sys,json; j=json.load(sys.stdin); print((j.get('result') or {}).get('object_ids',[''])[0])")"
  [[ -n "$cwl_id" ]] || die "smoke CWL ingest missing object_id"
  SMOKE_CWL_URL="${TES_BASE}/ga4gh/drs/v1/objects/${cwl_id}/stream"
  ok "using DRS stream workflow URL ($cwl_id)"
fi
SMOKE_CWL_URL="${SMOKE_CWL_URL:-https://raw.githubusercontent.com/SynapticFour/Ferrum/main/profiles/pipeline/fixtures/smoke-hello.cwl}"
WORKFLOW="$(python3 -c "import json; print(json.dumps({'workflow_type':'CWL','workflow_type_version':'v1.0','workflow_url':'${SMOKE_CWL_URL}','workflow_params':{'message':'smoke-pilot'}}))")"
run_json="$(curl -fsS -H 'Content-Type: application/json' -d "$WORKFLOW" "$BASE_URL/ga4gh/wes/v1/runs")"
run_id="$(printf '%s' "$run_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("run_id",""))')"
[[ -n "$run_id" ]] || die "WES submit failed: $run_json"

tes_backend="$(curl -fsS "$BASE_URL/admin/config" | python3 -c "import json,sys; print(json.load(sys.stdin).get('compute',{}).get('tes_backend',''))" 2>/dev/null || true)"
[[ "$tes_backend" == "docker" ]] || die "expected docker TES backend for up-tes (got: $tes_backend)"
ok "WES run $run_id tes_backend=$tes_backend"

state=""
poll_max=45
if [[ "${SMOKE_REQUIRE_COMPLETE:-}" == "1" || -n "${GITHUB_ACTIONS:-}" ]]; then
  poll_max=90
fi
for _ in $(seq 1 "$poll_max"); do
  state="$(curl -fsS "$BASE_URL/ga4gh/wes/v1/runs/${run_id}/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("state",""))' 2>/dev/null || true)"
  case "$state" in
    COMPLETE|CANCELED|EXECUTOR_ERROR|SYSTEM_ERROR) break ;;
  esac
  sleep 2
done
case "$state" in
  COMPLETE)
    ok "WES run reached COMPLETE"
    ;;
  EXECUTOR_ERROR)
    task_id="$(curl -fsS "$BASE_URL/ga4gh/tes/v1/tasks?limit=1" | python3 -c "import sys,json; t=json.load(sys.stdin).get('tasks',[]); print(t[0]['id'] if t else '')" 2>/dev/null || true)"
    stderr=""
    if [[ -n "$task_id" ]]; then
      stderr="$(curl -fsS "$BASE_URL/ga4gh/tes/v1/tasks/${task_id}" | python3 -c "import sys,json; logs=json.load(sys.stdin).get('logs',[]); print(logs[0].get('stderr','') if logs else '')" 2>/dev/null || true)"
    fi
    if [[ "${SMOKE_REQUIRE_COMPLETE:-}" == "1" || -n "${GITHUB_ACTIONS:-}" ]]; then
      die "WES run EXECUTOR_ERROR (CI requires COMPLETE) — $stderr"
    fi
    if printf '%s' "$stderr" | grep -q 'docker.sock'; then
      ok "WES submit OK; TES worker lacks docker.sock (set FERRUM_TES_DOCKER_MOUNT_SOCKET=1)"
    elif printf '%s' "$stderr" | grep -q 'client version'; then
      ok "WES submit OK; docker API mismatch in cwltool image (mount host docker CLI via DOCKER_BIN)"
    elif printf '%s' "$stderr" | grep -qE 'host_mnt|invalid mount config'; then
      ok "WES submit OK; nested docker bind paths differ on Docker Desktop Mac (CI/Linux expected COMPLETE)"
    else
      die "WES run EXECUTOR_ERROR — $stderr"
    fi
    ;;
  *)
    if [[ "${SMOKE_REQUIRE_COMPLETE:-}" == "1" || -n "${GITHUB_ACTIONS:-}" ]]; then
      die "WES run unexpected state=$state (CI requires COMPLETE) — fixture URL: $SMOKE_CWL_URL"
    fi
    die "WES run unexpected state=$state — fixture URL: $SMOKE_CWL_URL"
    ;;
esac

echo "smoke-pilot-local: all checks passed"
