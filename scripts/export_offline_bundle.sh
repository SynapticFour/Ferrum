#!/usr/bin/env bash
set -euo pipefail

OUTPUT_DIR="./offline-bundle"
COMPOSE_FILE="deploy/docker-compose.yml"
GATEWAY_IMAGE="ferrum-gateway:latest"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    --compose-file) COMPOSE_FILE="$2"; shift 2 ;;
    --gateway-image) GATEWAY_IMAGE="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

mkdir -p "${OUTPUT_DIR}"
ARCHIVE="${OUTPUT_DIR}/images-$(date +%Y%m%d-%H%M%S).tar"

echo "Pulling/building required images..."
docker compose -f "${COMPOSE_FILE}" build ferrum-gateway ferrum-ui ferrum-init
docker pull postgres:16-alpine
docker pull minio/minio:latest
docker pull keycloak/keycloak:26.0
docker pull nginx:alpine

echo "Saving images..."
docker save \
  "${GATEWAY_IMAGE}" \
  ferrum-ui:latest \
  ferrum-init:latest \
  postgres:16-alpine \
  minio/minio:latest \
  keycloak/keycloak:26.0 \
  nginx:alpine \
  -o "${ARCHIVE}"

gzip -f "${ARCHIVE}"

cat > "${OUTPUT_DIR}/manifest.txt" <<EOF
compose_file=${COMPOSE_FILE}
gateway_image=${GATEWAY_IMAGE}
generated_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

(
  cd "${OUTPUT_DIR}"
  shasum -a 256 ./* > checksums.sha256
)

echo "Offline bundle ready: ${OUTPUT_DIR}"

