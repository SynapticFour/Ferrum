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

# --- 1. Run DB migrations (journal-tracked; safe on partial / re-used volumes) ---
# Demo gateway sets FERRUM_DATABASE__RUN_MIGRATIONS=false and relies on this init job.
# Re-running every *.up.sql blindly fails when later migrations use plain CREATE TABLE
# (e.g. passport_visa_grants) on an already-initialised volume. Track applied files in
# _ferrum_init_migrations; bootstrap from _sqlx_migrations or existing schema when upgrading.
echo "Running database migrations..."
MIGRATIONS_DIR="${MIGRATIONS_DIR:-/migrations}"
PGHOST="${POSTGRES_HOST:-postgres}"
PGPORT="${POSTGRES_PORT:-5432}"
PGUSER="${POSTGRES_USER:-ferrum}"
PGDATABASE="${POSTGRES_DB:-ferrum}"
export PGPASSWORD="${POSTGRES_PASSWORD}"

psql_init() {
  psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -v ON_ERROR_STOP=1 "$@"
}

migration_version() {
  basename "$1" | cut -d_ -f1
}

migration_applied() {
  psql_init -t -A -c "SELECT COUNT(*) FROM _ferrum_init_migrations WHERE version = $1;" 2>/dev/null | tr -d ' '
}

record_migration() {
  psql_init -c "INSERT INTO _ferrum_init_migrations (version, filename)
    VALUES ($1, '$2')
    ON CONFLICT (version) DO NOTHING;"
}

migration_legacy_applied() {
  version="$1"
  case "$version" in
    20250611000004)
      psql_init -t -A -c "SELECT CASE WHEN to_regclass('public.reference_genomes') IS NOT NULL THEN 1 ELSE 0 END;"
      ;;
    *)
      # Base demo schema through passports (and earlier Africa migrations) share this marker.
      psql_init -t -A -c "SELECT CASE WHEN to_regclass('public.passport_visa_grants') IS NOT NULL THEN 1 ELSE 0 END;"
      ;;
  esac
}

ensure_migration_journal() {
  psql_init <<'SQL'
CREATE TABLE IF NOT EXISTS _ferrum_init_migrations (
    version BIGINT PRIMARY KEY,
    filename TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SQL
}

bootstrap_migration_journal() {
  ensure_migration_journal

  if [ "${FERRUM_INIT_RESET_MIGRATIONS:-0}" = "1" ]; then
    echo "  WARNING: FERRUM_INIT_RESET_MIGRATIONS=1 — clearing migration journal (dev only)" >&2
    psql_init -c "TRUNCATE _ferrum_init_migrations;"
    return 0
  fi

  journal_count="$(psql_init -t -A -c "SELECT COUNT(*) FROM _ferrum_init_migrations;" 2>/dev/null | tr -d ' ')"
  if [ "${journal_count:-0}" != "0" ]; then
    return 0
  fi

  sqlx_table="$(psql_init -t -A -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = '_sqlx_migrations';" 2>/dev/null | tr -d ' ')"
  if [ "${sqlx_table:-0}" = "1" ]; then
    echo "  Bootstrapping migration journal from _sqlx_migrations..."
    psql_init <<'SQL'
INSERT INTO _ferrum_init_migrations (version, filename)
SELECT version, description
FROM _sqlx_migrations
WHERE success = true
ON CONFLICT (version) DO NOTHING;
SQL
    journal_count="$(psql_init -t -A -c "SELECT COUNT(*) FROM _ferrum_init_migrations;" 2>/dev/null | tr -d ' ')"
    if [ "${journal_count:-0}" != "0" ]; then
      return 0
    fi
  fi

  passport_exists="$(psql_init -t -A -c "SELECT CASE WHEN to_regclass('public.passport_visa_grants') IS NOT NULL THEN 1 ELSE 0 END;" 2>/dev/null | tr -d ' ')"
  if [ "${passport_exists:-0}" != "1" ]; then
    return 0
  fi

  echo "  Existing schema detected without journal — marking prior migrations applied (no destructive re-apply)..."
  for f in $(ls -1 "$MIGRATIONS_DIR"/*.up.sql 2>/dev/null | sort); do
    [ -f "$f" ] || continue
    version="$(migration_version "$f")"
    filename="$(basename "$f")"
    legacy="$(migration_legacy_applied "$version" | tr -d ' ')"
    if [ "${legacy:-0}" = "1" ]; then
      echo "    Bootstrapped skip: $filename"
      record_migration "$version" "$filename"
    else
      echo "    Pending (schema marker missing): $filename"
    fi
  done
}

bootstrap_migration_journal

for f in $(ls -1 "$MIGRATIONS_DIR"/*.up.sql 2>/dev/null | sort); do
  [ -f "$f" ] || continue
  version="$(migration_version "$f")"
  filename="$(basename "$f")"
  applied="$(migration_applied "$version")"
  if [ "${applied:-0}" = "1" ]; then
    echo "  Skipping $filename (already applied)"
    continue
  fi
  echo "  Applying $filename"
  psql_init -f "$f" || { echo "Migration failed: $f" >&2; exit 1; }
  record_migration "$version" "$filename"
done

# --- 2. Create MinIO bucket ---
echo "Creating MinIO bucket..."
if command -v mc >/dev/null 2>&1; then
  mc alias set local "http://${MINIO_HOST:-minio}:${MINIO_PORT:-9000}" "${MINIO_ROOT_USER:-minioadmin}" "${MINIO_ROOT_PASSWORD:-minioadmin}"
  mc mb "local/${MINIO_BUCKET:-ferrum}" --ignore-existing 2>/dev/null || true
else
  echo "  (mc not installed, skipping bucket create; ensure bucket exists)"
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
if command -v ferrum-node-keygen >/dev/null 2>&1; then
  CRYPT4GH_MASTER_KEY_ID="${CRYPT4GH_MASTER_KEY_ID:-node}" ferrum-node-keygen "$KEY_DIR"
elif command -v ferrum-crypt4gh >/dev/null 2>&1; then
  ferrum-crypt4gh generate --output-dir "$KEY_DIR" 2>/dev/null || true
elif command -v crypt4gh >/dev/null 2>&1; then
  crypt4gh keys generate --sk "$KEY_DIR/node.sec" --pk "$KEY_DIR/node.pub" --force 2>/dev/null || true
else
  echo "  (ferrum-node-keygen not in PATH; keys can be generated later)"
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
  ('demo-ga4gh-sample', 'GA4GH OpenAPI (URL)', 'External OpenAPI YAML at a public URL — not stored in Ferrum.', 0, 'application/yaml', false, '["openapi.yaml"]'::jsonb),
  ('demo-sample-bam', 'External alignment README (URL)', 'URL pointer only — not a BAM file. Use make seed-pilot for real pilot files on MinIO.', 0, 'text/plain', false, '[]'::jsonb),
  ('demo-sample-vcf', 'demo-sample.vcf (URL)', 'HelixTest/conformance placeholder: HTTPS URL reference classified as VCF for htsget.', 0, 'text/vcf', false, '[]'::jsonb),
  ('demo-bam-to-vcf-demo-bam-to-vcf-1.0-input', 'E2E workflow input', 'DRS object used by HelixTest E2E pipeline as input for demo-bam-to-vcf.', 0, 'application/octet-stream', false, '[]'::jsonb)
ON CONFLICT (id) DO NOTHING;

UPDATE drs_objects
SET mime_type = 'text/vcf',
    name = 'demo-sample.vcf (URL)',
    description = 'HelixTest/conformance placeholder: HTTPS URL reference classified as VCF for htsget.'
WHERE id = 'demo-sample-vcf';

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

-- Pasteur / pilot demo variant (GRCh37 chr22:2000 T>G)
INSERT INTO beacon_datasets (id, name, description, assembly_id)
VALUES ('pasteur-demo', 'Pasteur pilot demo', 'chr22:2000 T>G for UI walkthrough', 'GRCh37')
ON CONFLICT (id) DO NOTHING;

INSERT INTO beacon_variants (dataset_id, chromosome, start, "end", reference, alternate, variant_type)
SELECT 'pasteur-demo', 'chr22', 2000, 2000, 'T', 'G', 'SNV'
WHERE NOT EXISTS (
  SELECT 1 FROM beacon_variants
  WHERE dataset_id = 'pasteur-demo'
    AND chromosome = 'chr22'
    AND start = 2000
    AND reference = 'T'
    AND alternate = 'G'
);

-- Workspace for demo-user (so "make demo" shows a pre-created workspace)
INSERT INTO workspaces (id, name, description, owner_sub, slug, is_archived, settings)
VALUES ('demo-workspace-01', 'Demo Workspace', 'Pre-populated workspace for testing. Add data, cohorts, and run workflows.', 'demo-user', 'demo-workspace', false, '{}'::jsonb)
ON CONFLICT (id) DO NOTHING;

INSERT INTO workspace_members (workspace_id, sub, role, invited_by)
VALUES ('demo-workspace-01', 'demo-user', 'owner', 'demo-user')
ON CONFLICT (workspace_id, sub) DO NOTHING;

-- Demo cohort in workspace
INSERT INTO cohorts (id, name, description, owner_sub, workspace_id, sample_count, tags)
VALUES (
  'demo-cohort-01',
  'Demo cohort',
  'Pilot samples: microbench object plus optional URL catalog references. Run make seed-pilot for real VCF/BAM on MinIO.',
  'demo-user',
  'demo-workspace-01',
  2,
  '["demo","public-reference"]'::jsonb
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO cohort_samples (id, cohort_id, sample_id, drs_object_ids, phenotype, added_by)
VALUES
  (
    'NA12878',
    'demo-cohort-01',
    'NA12878',
    '["microbench-plain-v1"]'::jsonb,
    '{"sex":"female","ancestry":"CEU","sequencing_type":"WGS"}'::jsonb,
    'demo-user'
  ),
  (
    'demo-sample-microbench',
    'demo-cohort-01',
    'microbench-plain',
    '["microbench-plain-v1"]'::jsonb,
    '{"sequencing_type":"synthetic","tissue_type":"benchmark"}'::jsonb,
    'demo-user'
  )
ON CONFLICT (id) DO NOTHING;

-- Link seeded DRS objects to demo workspace (visible in workspace contents)
UPDATE drs_objects SET workspace_id = 'demo-workspace-01'
WHERE id IN (
  'test-object-1',
  'demo-sample-bam',
  'demo-sample-vcf',
  'demo-bam-to-vcf-demo-bam-to-vcf-1.0-input',
  'microbench-plain-v1'
);

INSERT INTO workspace_activity (id, workspace_id, sub, action, resource_type, resource_id, details)
VALUES
  ('act-demo-1', 'demo-workspace-01', 'demo-user', 'seed.completed', 'workspace', 'demo-workspace-01', '{"source":"init-demo"}'::jsonb),
  ('act-demo-2', 'demo-workspace-01', 'demo-user', 'cohort.created', 'cohort', 'demo-cohort-01', '{"name":"Demo cohort"}'::jsonb)
ON CONFLICT (id) DO NOTHING;
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

INSERT INTO trs_tools (id, name, description, organization, toolclass, meta_version)
VALUES (
  'tiny-germline-hc',
  'TinyGermlineHC',
  'Minimal GATK HaplotypeCaller workflow (Ferrum-GA4GH-Demo).',
  'Ferrum Demo',
  'Workflow',
  '2.0'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO trs_tool_versions (id, tool_id, name, created_at, updated_at)
VALUES ('tiny-germline-hc-1.0', 'tiny-germline-hc', '1.0', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

DELETE FROM trs_files WHERE tool_id = 'tiny-germline-hc' AND version_id = 'tiny-germline-hc-1.0' AND file_type = 'DESCRIPTOR';
INSERT INTO trs_files (tool_id, version_id, file_type, descriptor_type, content, url, created_at)
VALUES (
  'tiny-germline-hc',
  'tiny-germline-hc-1.0',
  'DESCRIPTOR',
  'WDL',
  NULL,
  'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl',
  NOW()
);

-- Additional engine examples (WDL, CWL, Nextflow, Snakemake) for UI / WES demos
INSERT INTO trs_tools (id, name, description, organization, toolclass, meta_version)
VALUES
  ('demo-nextflow-qc', 'Demo Nextflow QC', 'Minimal Nextflow workflow (public nf-core example).', 'Ferrum Demo', 'Workflow', '2.0'),
  ('demo-cwl-sort', 'Demo CWL sort', 'Inline CWL sort example for TRS/WES.', 'Ferrum Demo', 'Workflow', '2.0'),
  ('demo-snakemake-hello', 'Demo Snakemake', 'Minimal Snakemake workflow for engine coverage.', 'Ferrum Demo', 'Workflow', '2.0'),
  ('demo-wdl-hello', 'Demo WDL hello', 'Minimal Cromwell WDL workflow for engine coverage.', 'Ferrum Demo', 'Workflow', '2.0')
ON CONFLICT (id) DO NOTHING;

INSERT INTO trs_tool_versions (id, tool_id, name, created_at, updated_at)
VALUES
  ('demo-nextflow-qc-1.0', 'demo-nextflow-qc', '1.0', NOW(), NOW()),
  ('demo-cwl-sort-1.0', 'demo-cwl-sort', '1.0', NOW(), NOW()),
  ('demo-snakemake-hello-1.0', 'demo-snakemake-hello', '1.0', NOW(), NOW()),
  ('demo-wdl-hello-1.0', 'demo-wdl-hello', '1.0', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

DELETE FROM trs_files WHERE tool_id IN ('demo-nextflow-qc','demo-cwl-sort','demo-snakemake-hello','demo-wdl-hello');
INSERT INTO trs_files (tool_id, version_id, file_type, descriptor_type, content, url, created_at)
VALUES
  ('demo-nextflow-qc', 'demo-nextflow-qc-1.0', 'DESCRIPTOR', 'NFL',
   $nfl$#!/usr/bin/env nextflow
process hello {
  output:
    path 'hello.txt'
  script:
    '''
    echo "Ferrum Nextflow demo" > hello.txt
    '''
}
workflow {
  hello()
}
$nfl$,
   NULL, NOW()),
  ('demo-cwl-sort', 'demo-cwl-sort-1.0', 'DESCRIPTOR', 'CWL',
   $cwl$cwlVersion: v1.0
class: CommandLineTool
baseCommand: [sh, -c]
inputs:
  - id: cmd
    type: string
    default: "printf 'c\nb\na\n' | sort -o sorted.txt"
    inputBinding:
      position: 0
outputs:
  out:
    type: File
    outputBinding:
      glob: sorted.txt
$cwl$,
   NULL, NOW()),
  ('demo-snakemake-hello', 'demo-snakemake-hello-1.0', 'DESCRIPTOR', 'SMK',
   $smk$rule hello:
    output:
        "hello.txt"
    shell:
        "echo hello > {output}"
$smk$,
   NULL, NOW()),
  ('demo-wdl-hello', 'demo-wdl-hello-1.0', 'DESCRIPTOR', 'WDL',
   $wdl$version 1.0

workflow HelloWdl {
  call writeHello
  output {
    File hello = writeHello.hello_out
  }
}

task writeHello {
  command {
    echo "Ferrum WDL demo" > hello.txt
  }
  output {
    File hello_out = "hello.txt"
  }
}
$wdl$,
   NULL, NOW());
TRSSEED

# --- 5b. Refresh DRS object sizes from URL Content-Length (seed SQL uses 0 as placeholder) ---
echo "Updating DRS object sizes from source URLs..."
probe_url_size() {
  url="$1"
  size="$(curl -fsSIL --max-time 20 "$url" 2>/dev/null | awk 'tolower($1)=="content-length:"{print $2; exit}')"
  if [ -n "$size" ] && [ "$size" -gt 0 ] 2>/dev/null; then
    echo "$size"
    return 0
  fi
  size="$(curl -fsSL --max-time 30 -o /dev/null -w '%{size_download}' "$url" 2>/dev/null)"
  if [ -n "$size" ] && [ "$size" -gt 0 ] 2>/dev/null; then
    echo "$size"
    return 0
  fi
  return 1
}
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -F $'\t' -c "
  SELECT o.id, sr.storage_key
  FROM drs_objects o
  JOIN storage_references sr ON sr.object_id = o.id
  WHERE sr.storage_backend = 'url' AND (o.size IS NULL OR o.size = 0);
" 2>/dev/null | while IFS=$'\t' read -r obj_id url; do
  [ -n "$obj_id" ] || continue
  [ -n "$url" ] || continue
  if size="$(probe_url_size "$url")"; then
    PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 -c \
      "UPDATE drs_objects SET size = ${size} WHERE id = '${obj_id}';" >/dev/null
    echo "  size ${obj_id}: ${size} bytes"
  else
    echo "  size ${obj_id}: could not probe URL (left as 0)"
  fi
done

# --- 6b. DRS /stream microbenchmark (MinIO + Postgres, last — repairs partial init / DO NOTHING skips) ---
# `GET .../objects/microbench-plain-v1/stream` for CI (see docs/PERFORMANCE-CRYPT4GH.md). Runs AFTER all DRS URL seeds.
MICRO_ID="microbench-plain-v1"
MICRO_KEY="microbench/plain-v1.bin"
GATEWAY_PUBLIC_URL="${GATEWAY_PUBLIC_URL:-http://localhost:8080}"
if command -v mc >/dev/null 2>&1; then
  mc alias set local "http://${MINIO_HOST:-minio}:${MINIO_PORT:-9000}" "${MINIO_ROOT_USER:-minioadmin}" "${MINIO_ROOT_PASSWORD:-minioadmin}" >/dev/null 2>&1 || true
  TMP_MB="/tmp/ferrum-microbench-plain.bin"
  if ! dd if=/dev/zero bs=4096 count=1 2>/dev/null | tr '\0' 'P' > "$TMP_MB" 2>/dev/null; then
    echo "ERROR: could not build microbench payload at $TMP_MB" >&2
    exit 1
  fi
  MB_SHA256=$(sha256sum "$TMP_MB" | awk '{print $1}')
  UPLOAD_OK=0
  i=1
  while [ "$i" -le 25 ]; do
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
  echo "  Microbench $MICRO_ID: MinIO ok, sha256=$MB_SHA256"
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
ON CONFLICT (id) DO UPDATE SET
  name = EXCLUDED.name,
  description = EXCLUDED.description,
  size = EXCLUDED.size,
  mime_type = EXCLUDED.mime_type,
  is_bundle = EXCLUDED.is_bundle,
  aliases = EXCLUDED.aliases;

INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
VALUES ('${MICRO_ID}', 's3', '${MICRO_KEY}', false)
ON CONFLICT (object_id) DO UPDATE SET
  storage_backend = EXCLUDED.storage_backend,
  storage_key = EXCLUDED.storage_key,
  is_encrypted = EXCLUDED.is_encrypted;

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
ON CONFLICT (object_id, type) DO UPDATE SET
  access_id = EXCLUDED.access_id,
  access_url = EXCLUDED.access_url,
  headers = EXCLUDED.headers;

INSERT INTO drs_checksums (object_id, type, checksum)
VALUES ('${MICRO_ID}', 'sha256', '${MB_SHA256}')
ON CONFLICT (object_id, type)
DO UPDATE SET checksum = EXCLUDED.checksum;
SEEDMICRO
  MB_ROWS=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -c "SELECT COUNT(*) FROM storage_references WHERE object_id='${MICRO_ID}';" 2>/dev/null || echo "0")
  if [ "$(echo "$MB_ROWS" | tr -d ' ')" != "1" ]; then
    echo "ERROR: microbench storage_references row missing after seed (count=$MB_ROWS)" >&2
    exit 1
  fi
  PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 -c \
    "UPDATE drs_objects SET workspace_id = 'demo-workspace-01' WHERE id = '${MICRO_ID}';" >/dev/null || true
else
  echo "  (mc not installed: skipping microbench-plain-v1 — DRS /stream microbench CI will fail unless you use deploy/Dockerfile.init)"
fi

# --- 6c. Reconcile demo workspace links (idempotent; fixes partial re-seeds) ---
echo "Linking seeded objects to demo workspace..."
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 <<'WSRECON'
UPDATE drs_objects SET workspace_id = 'demo-workspace-01'
WHERE id IN (
  'test-object-1',
  'demo-1000genomes-chr22',
  'demo-ena-run',
  'demo-ga4gh-sample',
  'demo-sample-bam',
  'demo-sample-vcf',
  'demo-bam-to-vcf-demo-bam-to-vcf-1.0-input',
  'microbench-plain-v1'
);

INSERT INTO wes_runs (
  run_id, workflow_url, workflow_type, workflow_type_version,
  workflow_params, state, workspace_id, start_time, end_time, tags, outputs
)
VALUES (
  'demo-run-seed-01',
  'https://raw.githubusercontent.com/SynapticFour/Ferrum-GA4GH-Demo/main/workflows/tiny_hc.wdl',
  'WDL',
  '1.0',
  '{"TinyGermlineHC.interval":"chr22:1700-2300"}'::jsonb,
  'COMPLETE',
  'demo-workspace-01',
  NOW() - INTERVAL '2 days',
  NOW() - INTERVAL '2 days' + INTERVAL '18 minutes',
  '{"source":"init-demo","label":"Demo germline run"}'::jsonb,
  '{
    "result_drs_id": "demo-sample-vcf",
    "output_files": [
      {
        "file_id": "demo-sample-vcf",
        "name": "Demo VCF output",
        "size": 0,
        "location": "drs://localhost/demo-sample-vcf"
      }
    ]
  }'::jsonb
)
ON CONFLICT (run_id) DO UPDATE SET
  workspace_id = EXCLUDED.workspace_id,
  state = EXCLUDED.state,
  outputs = EXCLUDED.outputs;

-- Demo run outputs (DRS-linked VCF for UI results tab; noop TES does not produce real files)
UPDATE wes_runs SET outputs = '{
  "result_drs_id": "demo-sample-vcf",
  "output_files": [
    {
      "file_id": "demo-sample-vcf",
      "name": "Demo VCF output",
      "size": 0,
      "location": "drs://localhost/demo-sample-vcf"
    }
  ]
}'::jsonb
WHERE run_id = 'demo-run-seed-01' AND (outputs IS NULL OR outputs = '{}'::jsonb);

INSERT INTO workspace_activity (id, workspace_id, sub, action, resource_type, resource_id, details)
VALUES
  ('act-demo-3', 'demo-workspace-01', 'demo-user', 'run.completed', 'wes_run', 'demo-run-seed-01', '{"workflow":"tiny_hc.wdl"}'::jsonb)
ON CONFLICT (id) DO NOTHING;

-- Lineage graph for UI (demo run consumed microbench, produced E2E URL output object)
INSERT INTO provenance_edges (id, from_type, from_id, to_type, to_id, edge_type, metadata)
VALUES
  ('prov-seed-in-01', 'drs_object', 'microbench-plain-v1', 'wes_run', 'demo-run-seed-01', 'input', '{}'::jsonb),
  ('prov-seed-out-01', 'wes_run', 'demo-run-seed-01', 'drs_object', 'demo-sample-vcf', 'output', '{}'::jsonb)
ON CONFLICT (id) DO NOTHING;
WSRECON

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
WS_OBJ_COUNT=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -t -A -c "SELECT COUNT(*) FROM drs_objects WHERE workspace_id = 'demo-workspace-01';" 2>/dev/null || echo "0")
echo "  demo-workspace-01: ${WS_OBJ_COUNT} linked DRS objects"
if [ "${WS_OBJ_COUNT:-0}" -lt 1 ]; then
  echo "WARNING: Demo workspace has no linked DRS objects." >&2
fi

echo "Init complete."
exit 0
