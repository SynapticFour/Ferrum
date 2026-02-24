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
  PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -f "$f" || true
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
if command -v ferrum-crypt4gh >/dev/null 2>&1; then
  ferrum-crypt4gh generate --output-dir "$KEY_DIR" 2>/dev/null || true
elif command -v crypt4gh >/dev/null 2>&1; then
  crypt4gh keys generate --name node.key --force 2>/dev/null && mv node.key node.key.pub "$KEY_DIR/" 2>/dev/null || true
else
  echo "  (crypt4gh not in PATH; keys can be generated later)"
fi

# --- 5. Seed example DRS objects (public genomic test data URLs) ---
echo "Seeding example DRS objects..."
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER:-ferrum}" -d "${POSTGRES_DB:-ferrum}" -v ON_ERROR_STOP=1 <<'SEED'
INSERT INTO drs_objects (id, name, description, size, mime_type, is_bundle, aliases)
VALUES
  ('demo-1000genomes-chr22', '1000 Genomes chr22 example', 'Public 1000 Genomes test data (URL)', 0, 'text/plain', false, '[]'::jsonb),
  ('demo-ena-run', 'ENA run XML example', 'European Nucleotide Archive run XML (URL)', 0, 'application/xml', false, '[]'::jsonb),
  ('demo-ga4gh-sample', 'GA4GH sample metadata example', 'Public sample metadata (URL)', 0, 'application/yaml', false, '[]'::jsonb)
ON CONFLICT (id) DO NOTHING;

INSERT INTO storage_references (object_id, storage_backend, storage_key, is_encrypted)
VALUES
  ('demo-1000genomes-chr22', 'url', 'https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README', false),
  ('demo-ena-run', 'url', 'https://ftp.ebi.ac.uk/pub/databases/ena/doc/example_run.xml', false),
  ('demo-ga4gh-sample', 'url', 'https://raw.githubusercontent.com/ga4gh-discovery/ga4gh-search/master/openapi.yaml', false)
ON CONFLICT (object_id) DO NOTHING;

INSERT INTO drs_access_methods (object_id, type, access_id, access_url, headers)
VALUES
  ('demo-1000genomes-chr22', 'https', 'access-demo-1000genomes-chr22', '{"url":"https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/release/20130502/README_chr22.20130502.README"}'::jsonb, '[]'::jsonb),
  ('demo-ena-run', 'https', 'access-demo-ena-run', '{"url":"https://ftp.ebi.ac.uk/pub/databases/ena/doc/example_run.xml"}'::jsonb, '[]'::jsonb),
  ('demo-ga4gh-sample', 'https', 'access-demo-ga4gh-sample', '{"url":"https://raw.githubusercontent.com/ga4gh-discovery/ga4gh-search/master/openapi.yaml"}'::jsonb, '[]'::jsonb)
ON CONFLICT (object_id, type) DO NOTHING;
SEED

echo "Init complete."
exit 0
