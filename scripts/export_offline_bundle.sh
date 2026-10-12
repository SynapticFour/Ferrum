#!/usr/bin/env bash
set -euo pipefail

# Offline bundle: Ferrum gateway/UI/init images plus postgres, SeaweedFS (S3),
# keycloak, and nginx. MinIO is not bundled (BYO). See docs/TESTED-S3-BACKENDS.md.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=/dev/null
COMPOSE_CMD="$(bash "${ROOT}/deploy/scripts/compose-cmd.sh")"

# Registry pulls fail transiently on shared CI runners. Retry each pull 3 times, 5 s apart.
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

POSTGRES_IMAGE="postgres:16-alpine@sha256:721873c34ceb9f8d8fc265984940dc982404c105f19ad51be9fdc5970a6080ea"
OBJECT_STORE_IMAGE="chrislusf/seaweedfs:3.80@sha256:1055999e08eed1789b0ae45d235126e4495e23d3fb9d6396293fd42539b1ae6a"
KEYCLOAK_IMAGE="keycloak/keycloak:26.0@sha256:09a381c715ab0b111835b70f2905955274843a219c6f27efb348e4d9f4086858"
NGINX_IMAGE="nginx:alpine@sha256:df221db836e1754089190208cee7eeda94f233197056426eda74a43ab1abeac2"

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
# shellcheck disable=SC2086
${COMPOSE_CMD} -f "${COMPOSE_FILE}" build ferrum-gateway ferrum-ui ferrum-init
docker_pull_retry "${POSTGRES_IMAGE}"
# SeaweedFS reference image is linux/amd64 in the digest we pin.
pull_object_store() {
  local attempt
  for attempt in 1 2 3; do
    if docker pull --platform linux/amd64 "${OBJECT_STORE_IMAGE}"; then
      return 0
    fi
    echo "docker pull ${OBJECT_STORE_IMAGE} failed (attempt ${attempt}/3)" >&2
    [[ "${attempt}" -lt 3 ]] && sleep 5
  done
  return 1
}
pull_object_store
docker_pull_retry "${KEYCLOAK_IMAGE}"
docker_pull_retry "${NGINX_IMAGE}"

echo "Saving images..."
docker save \
  "${GATEWAY_IMAGE}" \
  "${UI_IMAGE}" \
  "${INIT_IMAGE}" \
  "${POSTGRES_IMAGE}" \
  "${OBJECT_STORE_IMAGE}" \
  "${KEYCLOAK_IMAGE}" \
  "${NGINX_IMAGE}" \
  -o "${ARCHIVE}"

gzip -f "${ARCHIVE}"

cat > "${OUTPUT_DIR}/manifest.txt" <<EOF
ferrum_version=${VERSION}
compose_file=${COMPOSE_FILE}
gateway_image=${GATEWAY_IMAGE}
ui_image=${UI_IMAGE}
init_image=${INIT_IMAGE}
object_store_image=${OBJECT_STORE_IMAGE}
postgres_image=${POSTGRES_IMAGE}
generated_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

(
  cd "${OUTPUT_DIR}"
  shasum -a 256 ./* > checksums.sha256
)

echo "Offline bundle ready: ${OUTPUT_DIR}"
