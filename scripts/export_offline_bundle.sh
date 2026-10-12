#!/usr/bin/env bash
set -euo pipefail

# Offline bundle contents: Ferrum gateway/UI/init images plus postgres, the MinIO *server* image,
# keycloak, and nginx. The MinIO client (`mc`) is NOT required or bundled: the init script waits
# for MinIO with `nc`, and the MinIO server image is minimal (no curl, no mc).

# Registry pulls fail transiently on shared CI runners. Retry each pull 3 times, 5 s apart.
# A persistent failure still fails the script (set -e); there is no silent fallback.
docker_pull_retry() {
  local image="$1" attempt
  for attempt in 1 2 3; do
    if docker pull "${image}"; then
      return 0
    fi
    echo "docker pull ${image} failed (attempt ${attempt}/3)" >&2
    if [[ "${attempt}" -lt 3 ]]; then
      sleep 5
    fi
  done
  echo "error: docker pull ${image} failed after 3 attempts" >&2
  return 1
}

OUTPUT_DIR="./offline-bundle"
COMPOSE_FILE="deploy/docker-compose.yml"
VERSION="${FERRUM_VERSION:-latest}"
GATEWAY_IMAGE="ferrum-gateway:${VERSION}"
UI_IMAGE="ferrum-ui:${VERSION}"
INIT_IMAGE="ferrum-init:${VERSION}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    --compose-file) COMPOSE_FILE="$2"; shift 2 ;;
    --version) VERSION="$2"; GATEWAY_IMAGE="ferrum-gateway:${VERSION}"; UI_IMAGE="ferrum-ui:${VERSION}"; INIT_IMAGE="ferrum-init:${VERSION}"; shift 2 ;;
    --gateway-image) GATEWAY_IMAGE="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

mkdir -p "${OUTPUT_DIR}"
ARCHIVE="${OUTPUT_DIR}/images-${VERSION}.tar"

echo "Pulling/building required images (version ${VERSION})..."
export FERRUM_VERSION="${VERSION}"
docker compose -f "${COMPOSE_FILE}" build ferrum-gateway ferrum-ui ferrum-init
docker_pull_retry postgres:16-alpine
docker_pull_retry minio/minio:RELEASE.2024-12-18T13-15-44Z
docker_pull_retry keycloak/keycloak:26.0
docker_pull_retry nginx:alpine

echo "Saving images..."
docker save \
  "${GATEWAY_IMAGE}" \
  "${UI_IMAGE}" \
  "${INIT_IMAGE}" \
  postgres:16-alpine \
  minio/minio:RELEASE.2024-12-18T13-15-44Z \
  keycloak/keycloak:26.0 \
  nginx:alpine \
  -o "${ARCHIVE}"

gzip -f "${ARCHIVE}"

cat > "${OUTPUT_DIR}/manifest.txt" <<EOF
ferrum_version=${VERSION}
compose_file=${COMPOSE_FILE}
gateway_image=${GATEWAY_IMAGE}
ui_image=${UI_IMAGE}
init_image=${INIT_IMAGE}
generated_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

(
  cd "${OUTPUT_DIR}"
  shasum -a 256 ./* > checksums.sha256
)

echo "Offline bundle ready: ${OUTPUT_DIR}"
