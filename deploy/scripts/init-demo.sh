#!/usr/bin/env sh
# Ferrum demo init: migrations, MinIO bucket, Keycloak realm, Crypt4GH keys, seed DRS objects.
set -e

# --- Wait for dependencies ---
wait_for() {
  host="$1"; port="$2"; name="$3"
  for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    if nc -z "$host" "$port" 2>/dev/null; then
      echo "$name is ready."
      return 0
    fi
    echo "Waiting for $name at $host:$port ..."
    sleep 2
  done
  echo "Timeout waiting for $name" >&2
  return 1
}

wait_for "${POSTGRES_HOST:-postgres}" "${POSTGRES_PORT:-5432}" "PostgreSQL"
wait_for "${MINIO_HOST:-minio}" "${MINIO_PORT:-9000}" "MinIO"
wait_for "${KEYCLOAK_HOST:-keycloak}" "${KEYCLOAK_PORT:-8080}" "Keycloak"

# --- 1. Run DB migrations ---
echo "Running database migrations..."
MIGRATIONS_DIR="${MIGRATIONS_DIR:-/migrations}"
for f in $(ls -1 "$MIGRATIONS_DIR"/*.up.sql 2>/dev/null | sort); do
  [ -f "$f" ] || continue
  echo "  Applying $(basename "$f")"
  PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 -f "$f" || { echo "Migration failed: $f" >&2; exit 1; }
done

# --- 2. Create MinIO bucket ---
echo "Creating MinIO bucket..."
if command -v mc >/dev/null 2>&1; then
  mc alias set local "http://${MINIO_HOST:-minio}:${MINIO_PORT:-9000}" "${MINIO_ROOT_USER:-minioadmin}" "${MINIO_ROOT_PASSWORD:-minioadmin}"
  mc mb "local/${MINIO_BUCKET:-ferrum}" --ignore-existing 2>/dev/null || true
else
  echo "  (mc not installed, skipping bucket create; ensure bucket exists)"
fi

# --- 2b. Stream microbenchmark object (MinIO + DRS, 4096-byte deterministic payload) ---
# Used by CI and external “Plain vs Crypt4GH” demos: `GET .../objects/microbench-plain-v1/stream`.
# See docs/PERFORMANCE-CRYPT4GH.md. Crypt4GH twin is created via ingest (same bytes, encrypt=true), not here.
# MinIO may need a short delay after TCP is open; retries avoid empty DB + missing blob (DRS /stream 500).
MICRO_ID="microbench-plain-v1"
MICRO_KEY="microbench/plain-v1.bin"
GATEWAY_PUBLIC_URL="${GATEWAY_PUBLIC_URL:-http://localhost:8080}"
if command -v mc >/dev/null 2>&1; then
  TMP_MB="/tmp/ferrum-microbench-plain.bin"
  if ! dd if=/dev/zero bs=4096 count=1 2>/dev/null | tr '\0' 'P' > "$TMP_MB" 2>/dev/null; then
    echo "ERROR: could not build microbench payload at $TMP_MB" >&2
    exit 1
  fi
  MB_SHA256=$(sha256sum "$TMP_MB" | awk '{print $1}')
  UPLOAD_OK=0
  i=1
  while [ "$i" -le 20 ]; do
    if mc cp "$TMP_MB" "local/${MINIO_BUCKET:-ferrum}/$MICRO_KEY" && mc stat "local/${MINIO_BUCKET:-ferrum}/$MICRO_KEY" >/dev/null 2>&1; then
      UPLOAD_OK=1
      break
    fi
    echo "  microbench: MinIO upload/stat attempt $i failed, retry in 2s..."
    sleep 2
    i=$((i + 1))
  done
  if [ "$UPLOAD_OK" != "1" ]; then
    echo "ERROR: microbench upload to MinIO failed after retries (bucket=${MINIO_BUCKET:-ferrum} key=$MICRO_KEY)" >&2
    exit 1
  fi
  echo "  Microbench object $MICRO_ID sha256=$MB_SHA256 (MinIO ok)"
  PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 <<SEEDMICRO
INSERT INTO drs_objects (id, name, description, size, mime_type, is_bundle, aliases)
VALUES (
  '${MICRO_ID}',
  'Microbench plaintext (S3)',
  'Deterministic 4096-byte payload on MinIO for DRS /stream timing (Plain path).',
  4096,
  'application/octet-stream',
  false,
  '[]'::jsonb
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
VALUES ('${MICRO_ID}', 's3', '${MICRO_KEY}', false)
ON CONFLICT (object_id) DO NOTHING;

INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers)
VALUES (
  '${MICRO_ID}',
  'https',
  'access-${MICRO_ID}',
  jsonb_build_object(
    'url',
    '${GATEWAY_PUBLIC_URL}/ga4gh/drs/v1/objects/${MICRO_ID}/access/access-${MICRO_ID}'
  ),
  '[]'::jsonb
)
ON CONFLICT (object_id, type) DO NOTHING;

INSERT INTO drs_checksums (object_id, type, checksum)
VALUES ('${MICRO_ID}', 'sha256', '${MB_SHA256}')
ON CONFLICT (object_id, type)
DO UPDATE SET checksum = EXCLUDED.checksum;
SEEDMICRO
else
  echo "  (mc not installed: skipping microbench-plain-v1 — DRS /stream microbench CI will fail unless you use deploy/Dockerfile.init)"
fi

# --- 3. Keycloak realm + test users ---
echo "Configuring Keycloak realm..."
KEYCLOAK_URL="${KEYCLOAK_URL:-http://keycloak:8080}"
ADMIN="${KEYCLOAK_ADMIN:-admin}"
ADMIN_PW="${KEYCLOAK_ADMIN_PASSWORD:-admin}"
REALM="${KEYCLOAK_REALM:-ferrum}"

# Get admin token
TOKEN=$(curl -s -X POST "$KEYCLOAK_URL/realms/master/protocol/openid-connect/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "username=$ADMIN" \
  -d "password=$ADMIN_PW" \
  -d "grant_type=password" \
  -d "client_id=admin-cli" \
  | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')

if [ -n "$TOKEN" ]; then
  # Create realm if not exists
  curl -s -o /dev/null -w "%{http_code}" -X POST "$KEYCLOAK_URL/admin/realms" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"realm\":\"$REALM\",\"enabled\":true}" || true

  # Create test user alice / alice
  curl -s -o /dev/null -X POST "$KEYCLOAK_URL/admin/realms/$REALM/users" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"username":"alice","enabled":true,"credentials":[{"type":"password","value":"alice","temporary":false}]}' || true

  # Create test user bob / bob
  curl -s -o /dev/null -X POST "$KEYCLOAK_URL/admin/realms/$REALM/users" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"username":"bob","enabled":true,"credentials":[{"type":"password","value":"bob","temporary":false}]}' || true
  echo "  Realm $REALM and users alice, bob configured."
else
  echo "  Could not get Keycloak token; skip realm setup."
fi

# --- 4. Crypt4GH keypair for the node ---
echo "Generating Crypt4GH keypair..."
KEY_DIR="${CRYPT4GH_KEY_DIR:-/data/ferrum/keys}"
mkdir -p "$KEY_DIR"
if command -v ferrum-crypt4gh >/dev/null 2>&1; then
  ferrum-crypt4gh generate --output-dir "$KEY_DIR" 2>/dev/null || true
elif command -v crypt4gh >/dev/null 2>&1; then
  crypt4gh keys generate --name node.key --force 2>/dev/null && mv node.key node.key.pub "$KEY_DIR/" 2>/dev/null || true
else
  echo "  (crypt4gh not in PATH; keys can be generated later)"
fi

# --- 5. Seed example DRS objects (public genomic test data URLs), workspace ---
echo "Seeding demo data (DRS, workspace)..."

# HelixTest strict DRS checksum validation expects `test-object-1` to expose a
# sha256 checksum matching the bytes downloaded from its `access_url.url`.
TEST_OBJECT_1_ACCESS_URL="https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md"
TEST_OBJECT_1_SHA256="$(curl -fsSL "$TEST_OBJECT_1_ACCESS_URL" | sha256sum | awk '{print $1}')"
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 <<'SEED'
-- DRS: existing + BAM/VCF-style examples (URLs to public test data)
INSERT INTO drs_objects (id, name, description, size, mime_type, is_bundle, aliases)
VALUES
  ('test-object-1', 'HelixTest object 1', 'Seed object for HelixTest (DRS + htsget reads/BAM class).', 0, 'application/vnd.ga4gh.bam', false, '[]'::jsonb),
  ('demo-1000genomes-chr22', '1000 Genomes chr22 example', 'Public 1000 Genomes test data (URL)', 0, 'text/plain', false, '[]'::jsonb),
  ('demo-ena-run', 'ENA run XML example', 'European Nucleotide Archive run XML (URL)', 0, 'application/xml', false, '[]'::jsonb),
  ('demo-ga4gh-sample', 'GA4GH sample metadata example', 'Public sample metadata (URL)', 0, 'application/yaml', false, '[]'::jsonb),
  ('demo-sample-bam', 'Demo BAM file', 'Example aligned reads (BAM) for demo', 0, 'application/octet-stream', false, '["demo.bam"]'::jsonb),
  ('demo-sample-vcf', 'Demo VCF file', 'Example variants (VCF) for demo', 0, 'text/vcf', false, '["demo.vcf"]'::jsonb),
  ('demo-bam-to-vcf-demo-bam-to-vcf-1.0-input', 'E2E workflow input', 'DRS object used by HelixTest E2E pipeline as input for demo-bam-to-vcf.', 0, 'application/octet-stream', false, '[]'::jsonb)
ON CONFLICT (id) DO NOTHING;

INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
VALUES
  ('test-object-1', 'url', 'https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md', false),
  ('demo-1000genomes-chr22', 'url', 'https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README', false),
  ('demo-ena-run', 'url', 'https://ftp.ebi.ac.uk/pub/databases/ena/doc/example_run.xml', false),
  ('demo-ga4gh-sample', 'url', 'https://raw.githubusercontent.com/ga4gh-discovery/ga4gh-search/master/openapi.yaml', false),
  ('demo-sample-bam', 'url', 'https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/data_collections/1000_genomes_project/data/CEU/NA12878/alignment/README', false),
  ('demo-sample-vcf', 'url', 'https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md', false),
  ('demo-bam-to-vcf-demo-bam-to-vcf-1.0-input', 'url', 'https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README', false)
ON CONFLICT (object_id) DO NOTHING;

INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers)
VALUES
  ('test-object-1', 'https', 'access-test-object-1', '{"url":"https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md"}'::jsonb, '[]'::jsonb),
  ('demo-1000genomes-chr22', 'https', 'access-demo-1000genomes-chr22', '{"url":"https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README"}'::jsonb, '[]'::jsonb),
  ('demo-ena-run', 'https', 'access-demo-ena-run', '{"url":"https://ftp.ebi.ac.uk/pub/databases/ena/doc/example_run.xml"}'::jsonb, '[]'::jsonb),
  ('demo-ga4gh-sample', 'https', 'access-demo-ga4gh-sample', '{"url":"https://raw.githubusercontent.com/ga4gh-discovery/ga4gh-search/master/openapi.yaml"}'::jsonb, '[]'::jsonb),
  ('demo-sample-bam', 'https', 'access-demo-bam', '{"url":"https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/data_collections/1000_genomes_project/data/CEU/NA12878/alignment/README"}'::jsonb, '[]'::jsonb),
  ('demo-sample-vcf', 'https', 'access-demo-vcf', '{"url":"https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md"}'::jsonb, '[]'::jsonb),
  ('demo-bam-to-vcf-demo-bam-to-vcf-1.0-input', 'https', 'access-demo-bam-to-vcf-input', '{"url":"https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README"}'::jsonb, '[]'::jsonb)
ON CONFLICT (object_id, type) DO NOTHING;

-- Align HelixTest htsget reads default (test-object-1) with BAM ticket schema even if row pre-existed
UPDATE drs_objects SET
  mime_type = 'application/vnd.ga4gh.bam',
  description = 'Seed object for HelixTest (DRS + htsget reads/BAM class).'
WHERE id = 'test-object-1';

-- --- Beacon v2 demo data (HelixTest expects a known variant exists and a negative coordinate does not) ---
INSERT INTO beacon_datasets (id, name, description, assembly_id)
VALUES ('default', 'Ferrum demo Beacon dataset', 'Seeded for HelixTest integration', 'GRCh38')
ON CONFLICT (id) DO NOTHING;

-- Positive: referenceName=1, start=1000, referenceBases=A, alternateBases=T
INSERT INTO beacon_variants (dataset_id, chromosome, start, "end", reference, alternate, variant_type)
SELECT 'default', 'chr1', 1000, 1000, 'A', 'T', 'SNV'
WHERE NOT EXISTS (
  SELECT 1 FROM beacon_variants
  WHERE dataset_id = 'default'
    AND chromosome = 'chr1'
    AND start = 1000
    AND "end" = 1000
    AND reference = 'A'
    AND alternate = 'T'
);

-- Negative is validated by absence: referenceName=1, start=999999999, referenceBases=C, alternateBases=G.

-- Workspace for demo-user (so "make demo" shows a pre-created workspace)
INSERT INTO workspaces (id, name, description, owner_sub, slug, is_archived, settings)
VALUES ('demo-workspace-01', 'Demo Workspace', 'Pre-populated workspace for testing. Add data, cohorts, and run workflows.', 'demo-user', 'demo-workspace', false, '{}'::jsonb)
ON CONFLICT (id) DO NOTHING;

INSERT INTO workspace_members (workspace_id, sub, role, invited_by)
VALUES ('demo-workspace-01', 'demo-user', 'owner', 'demo-user')
ON CONFLICT (workspace_id, sub) DO NOTHING;
SEED

# Insert sha256 checksum metadata for HelixTest conformance.
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 -c "
  INSERT INTO drs_checksums (object_id, type, checksum)
  VALUES ('test-object-1', 'sha256', '${TEST_OBJECT_1_SHA256}')
  ON CONFLICT (object_id, type)
  DO UPDATE SET checksum = EXCLUDED.checksum;
"

# --- 6. Seed TRS tool (required for HelixTest /tools/{id}/versions) ---
echo "Seeding TRS demo tool..."
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 <<'TRSSEED'
INSERT INTO trs_tools (id, name, description, organization, toolclass, meta_version)
VALUES ('demo-bam-to-vcf', 'BAM to VCF', 'Example tool: call variants from BAM (demo).', 'Ferrum Demo', 'Workflow', '2.0')
ON CONFLICT (id) DO NOTHING;

INSERT INTO trs_tool_versions (id, tool_id, name, created_at, updated_at)
VALUES ('demo-bam-to-vcf-1.0', 'demo-bam-to-vcf', '1.0', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- Ensure CWL (and PLAIN_CWL) descriptors for HelixTest GET .../descriptor/{type} (idempotent)
DELETE FROM trs_files WHERE tool_id = 'demo-bam-to-vcf' AND version_id = 'demo-bam-to-vcf-1.0' AND file_type = 'DESCRIPTOR';
INSERT INTO trs_files (tool_id, version_id, file_type, descriptor_type, content, url, created_at)
VALUES
  ('demo-bam-to-vcf', 'demo-bam-to-vcf-1.0', 'DESCRIPTOR', 'CWL', 'cwlVersion: v1.0\nclass: Workflow\ninputs:\n  bam: File\noutputs:\n  vcf: File\nsteps: []', NULL, NOW()),
  ('demo-bam-to-vcf', 'demo-bam-to-vcf-1.0', 'DESCRIPTOR', 'PLAIN_CWL', 'cwlVersion: v1.0\nclass: Workflow\ninputs:\n  bam: File\noutputs:\n  vcf: File\nsteps: []', NULL, NOW());
TRSSEED

# --- 7. Verify demo data ---
echo "Verifying demo data..."
VERIFY=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -c "
  SELECT (SELECT COUNT(*) FROM workspaces)::text || ' workspaces, ' ||
         (SELECT COUNT(*) FROM drs_objects)::text || ' DRS objects, ' ||
         (SELECT COUNT(*) FROM trs_tools)::text || ' TRS tools, ' ||
         (SELECT COUNT(*) FROM trs_files WHERE tool_id = 'demo-bam-to-vcf' AND version_id = 'demo-bam-to-vcf-1.0')::text || ' TRS descriptor rows'
  FROM (SELECT 1) x;
" 2>/dev/null || echo "0 workspaces, 0 DRS objects, 0 TRS tools, 0 TRS descriptor rows")
echo "  $VERIFY"
TRS_DESC_COUNT=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -c "SELECT COUNT(*) FROM trs_files WHERE tool_id = 'demo-bam-to-vcf' AND version_id = 'demo-bam-to-vcf-1.0';" 2>/dev/null || echo "0")
if [ "${TRS_DESC_COUNT:-0}" -lt 1 ]; then
  echo "ERROR: No TRS descriptor rows for demo-bam-to-vcf/demo-bam-to-vcf-1.0. HelixTest descriptor retrieval will 404." >&2
  exit 1
fi
WS_COUNT=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -c "SELECT COUNT(*) FROM workspaces;" 2>/dev/null || echo "0")
if [ "${WS_COUNT:-0}" -lt 1 ]; then
  echo "WARNING: No workspaces found after seed. Demo workspace may not be visible." >&2
fi

echo "Init complete."
exit 0
