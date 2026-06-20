#!/usr/bin/env bash
# Optional pilot enrichment: upload real small files to MinIO and wire cohort/lineage.
# Run AFTER the stack is up (make up / make up-tes). Idempotent — safe to re-run.
#
# init-demo.sh stays minimal (HelixTest/conformance + honest URL catalog).
# This script adds managed-storage objects analysts can preview and use in workflows.
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:8080}"
BASE_URL="${BASE_URL%/}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_DIR="${REPO_ROOT}/profiles/pipeline/fixtures"
FIXTURE_VCF="${FIXTURE_DIR}/tiny.vcf"
FIXTURE_BAM="${FIXTURE_DIR}/tiny.bam"
FIXTURE_BAI="${FIXTURE_DIR}/tiny.bam.bai"
REF_FASTA="${FIXTURE_DIR}/pilot-ref.fa"
REF_FAI="${FIXTURE_DIR}/pilot-ref.fa.fai"
TRUTH_VCF="${FIXTURE_DIR}/pilot-truth.vcf.gz"
TRUTH_TBI="${FIXTURE_DIR}/pilot-truth.vcf.gz.tbi"
WORKSPACE_ID="${PILOT_WORKSPACE_ID:-demo-workspace-01}"
COHORT_ID="${PILOT_COHORT_ID:-demo-cohort-01}"
SAMPLE_ID="${PILOT_SAMPLE_ID:-pilot-demo-01}"
PILOT_VCF_NAME="Pilot demo VCF (MinIO)"
PILOT_BAM_NAME="Pilot demo BAM (MinIO)"
PILOT_BAI_NAME="Pilot demo BAM index (MinIO)"
PILOT_REF_NAME="Pilot reference FASTA (MinIO)"
PILOT_REF_INDEX_NAME="Pilot reference FASTA index (MinIO)"
PILOT_TRUTH_NAME="Pilot truth VCF (MinIO)"
PILOT_TRUTH_INDEX_NAME="Pilot truth VCF index (MinIO)"

POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_USER="${POSTGRES_USER:-ferrum}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-ferrum}"
POSTGRES_DB="${POSTGRES_DB:-ferrum}"

die() { echo "seed-pilot-demo: $*" >&2; exit 1; }

# Bash 3.2 (macOS) + `set -u`: empty CURL_AUTH[@] is unbound — use a helper instead.
curl_pilot() {
  if [[ -n "${FERRUM_PASSPORT_JWT:-}" ]]; then
    curl -fsS -H "Authorization: Bearer ${FERRUM_PASSPORT_JWT}" "$@"
  else
    curl -fsS "$@"
  fi
}

curl_pilot "$BASE_URL/health" >/dev/null || die "Gateway not reachable at $BASE_URL (start stack first)"
[[ -f "$FIXTURE_VCF" ]] || die "Missing fixture: $FIXTURE_VCF"
[[ -f "$FIXTURE_BAM" && -f "$FIXTURE_BAI" ]] || die "Missing BAM fixtures — run: bash profiles/pipeline/fixtures/build-tiny-bam.sh"
[[ -f "$REF_FASTA" && -f "$REF_FAI" && -f "$TRUTH_VCF" && -f "$TRUTH_TBI" ]] || die "Missing reference bundle — run: bash profiles/pipeline/fixtures/build-pilot-ref-bundle.sh"

poll_ingest_job() {
  local job_id="$1"
  local status
  for _ in $(seq 1 60); do
    status="$(curl_pilot "$BASE_URL/api/v1/ingest/jobs/${job_id}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || true)"
    case "$status" in
      succeeded) return 0 ;;
      failed) die "ingest job $job_id failed" ;;
    esac
    sleep 1
  done
  die "ingest job $job_id timed out"
}

ingest_file() {
  local path="$1"
  local name="$2"
  local client_id="$3"
  local resp job_id object_id
  resp="$(curl_pilot -X POST "$BASE_URL/api/v1/ingest/upload" \
    -F "file=@${path}" \
    -F "name=${name}" \
    -F "client_request_id=${client_id}")"
  job_id="$(printf '%s' "$resp" | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")"
  poll_ingest_job "$job_id"
  object_id="$(curl_pilot "$BASE_URL/api/v1/ingest/jobs/${job_id}" | python3 -c "import sys,json; j=json.load(sys.stdin); print((j.get('result') or {}).get('object_ids',[''])[0])")"
  [[ -n "$object_id" ]] || die "no object_id from ingest job $job_id"
  printf '%s' "$object_id"
}

psql_exec() {
  if command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}' 2>/dev/null | grep -q postgres; then
    local c
    c="$(docker ps --format '{{.Names}}' | grep postgres | head -1)"
    docker exec -i "$c" psql -q -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 "$@"
  elif command -v psql >/dev/null 2>&1; then
    PGPASSWORD="$POSTGRES_PASSWORD" psql -q -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 "$@"
  else
    die "need docker postgres container or local psql to link workspace/cohort"
  fi
}

lookup_object_by_name() {
  local display_name="$1"
  psql_exec -t -A -c "SELECT id FROM drs_objects WHERE name = '${display_name}' LIMIT 1;" 2>/dev/null | tr -d '[:space:]' || true
}

seed_named_object() {
  local display_name="$1"
  local fixture_path="$2"
  local upload_name="$3"
  local client_id="$4"
  local description="$5"
  local mime_type="$6"
  local existing_id
  existing_id="$(lookup_object_by_name "$display_name")"
  if [[ -n "$existing_id" ]]; then
    echo "  already seeded: ${existing_id} (${display_name})" >&2
    printf '%s' "$existing_id"
    return 0
  fi
  existing_id="$(ingest_file "$fixture_path" "$upload_name" "$client_id")"
  psql_exec -c "
    UPDATE drs_objects
    SET name = '${display_name}',
        description = '${description}',
        mime_type = '${mime_type}',
        workspace_id = '${WORKSPACE_ID}'
    WHERE id = '${existing_id}';
  "
  echo "  uploaded → ${existing_id} (${display_name})" >&2
  printf '%s' "$existing_id"
}

echo "==> Pilot VCF on managed storage"
VCF_ID="$(seed_named_object \
  "$PILOT_VCF_NAME" \
  "$FIXTURE_VCF" \
  "pilot-demo.vcf" \
  "pilot-seed-vcf-v1" \
  "Small VCF on MinIO — inline preview and workflow input." \
  "text/vcf")"

echo "==> Pilot BAM + BAI on managed storage"
BAM_ID="$(seed_named_object \
  "$PILOT_BAM_NAME" \
  "$FIXTURE_BAM" \
  "pilot-demo.bam" \
  "pilot-seed-bam-v1" \
  "Indexed BAM on MinIO — germline workflow input." \
  "application/vnd.ga4gh.bam")"

BAI_ID="$(seed_named_object \
  "$PILOT_BAI_NAME" \
  "$FIXTURE_BAI" \
  "pilot-demo.bam.bai" \
  "pilot-seed-bai-v1" \
  "BAM index (.bai) for pilot-demo.bam." \
  "application/octet-stream")"

echo "==> Pilot reference bundle (TinyGermlineHC ref + truth)"
REF_ID="$(seed_named_object \
  "$PILOT_REF_NAME" \
  "$REF_FASTA" \
  "pilot-ref.fa" \
  "pilot-seed-ref-v1" \
  "chr22 stub FASTA for germline workflow inputs." \
  "application/x-fasta")"

REF_FAI_ID="$(seed_named_object \
  "$PILOT_REF_INDEX_NAME" \
  "$REF_FAI" \
  "pilot-ref.fa.fai" \
  "pilot-seed-ref-fai-v1" \
  "FASTA index (.fai) for pilot reference." \
  "application/octet-stream")"

TRUTH_ID="$(seed_named_object \
  "$PILOT_TRUTH_NAME" \
  "$TRUTH_VCF" \
  "pilot-truth.vcf.gz" \
  "pilot-seed-truth-v1" \
  "Gzipped truth VCF for TinyGermlineHC --alleles." \
  "application/gzip")"

TRUTH_TBI_ID="$(seed_named_object \
  "$PILOT_TRUTH_INDEX_NAME" \
  "$TRUTH_TBI" \
  "pilot-truth.vcf.gz.tbi" \
  "pilot-seed-truth-tbi-v1" \
  "Tabix index (.tbi) for pilot truth VCF." \
  "application/octet-stream")"

echo "==> Cohort + lineage wiring"
psql_exec <<SQL
UPDATE drs_objects SET workspace_id = '${WORKSPACE_ID}'
WHERE id IN ('${VCF_ID}', '${BAM_ID}', '${BAI_ID}', '${REF_ID}', '${REF_FAI_ID}', '${TRUTH_ID}', '${TRUTH_TBI_ID}', 'microbench-plain-v1');

INSERT INTO cohort_samples (id, cohort_id, sample_id, drs_object_ids, phenotype, added_by)
VALUES (
  'pilot-sample-vcf',
  '${COHORT_ID}',
  '${SAMPLE_ID}',
  '["${BAM_ID}", "${BAI_ID}", "${VCF_ID}"]'::jsonb,
  '{"sequencing_type":"WGS","source":"seed-pilot-demo","sex":"female"}'::jsonb,
  'demo-user'
)
ON CONFLICT (id) DO UPDATE SET
  drs_object_ids = EXCLUDED.drs_object_ids,
  phenotype = EXCLUDED.phenotype;

INSERT INTO provenance_edges (id, from_type, from_id, to_type, to_id, edge_type, metadata)
VALUES
  ('prov-pilot-vcf-01', 'drs_object', 'microbench-plain-v1', 'wes_run', 'demo-run-seed-01', 'input', '{"source":"seed-pilot-demo"}'::jsonb),
  ('prov-pilot-vcf-02', 'wes_run', 'demo-run-seed-01', 'drs_object', '${VCF_ID}', 'output', '{"source":"seed-pilot-demo"}'::jsonb),
  ('prov-pilot-bam-01', 'drs_object', '${BAM_ID}', 'wes_run', 'demo-run-seed-01', 'input', '{"source":"seed-pilot-demo","role":"alignment"}'::jsonb)
ON CONFLICT (id) DO NOTHING;
SQL

echo "==> Pilot seed complete"
echo "  VCF: ${VCF_ID} — inline preview in Data Browser"
echo "  BAM: ${BAM_ID} + index ${BAI_ID} — cohort germline inputs"
echo "  Ref: ${REF_ID} + index ${REF_FAI_ID} — TinyGermlineHC ref_fasta inputs"
echo "  Truth: ${TRUTH_ID} + index ${TRUTH_TBI_ID} — TinyGermlineHC truth inputs"
echo "  Cohort: ${SAMPLE_ID} in ${COHORT_ID}"
