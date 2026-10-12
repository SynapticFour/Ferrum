#!/usr/bin/env bash
# Build release Docker images, offline bundle, and Helm chart for GitHub Release.
# Requires: VERSIONS.lock loaded (GA4GH_INFRA_REF), FERRUM_RELEASE_VERSION set (e.g. v0.2.0).
# Offline image export is best-effort when called from CI with FERRUM_OFFLINE_BUNDLE_OPTIONAL=1.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${FERRUM_RELEASE_VERSION:?set FERRUM_RELEASE_VERSION (e.g. v0.2.0)}"
GA4GH_INFRA_REF="${GA4GH_INFRA_REF:?load VERSIONS.lock first}"
STAGING="${RELEASE_STAGING_DIR:-release-staging}"
COMPOSE_FILE="${COMPOSE_FILE:-deploy/docker-compose.yml}"
COMPOSE_CMD="$(bash "${ROOT}/deploy/scripts/compose-cmd.sh")"
OPTIONAL_OFFLINE="${FERRUM_OFFLINE_BUNDLE_OPTIONAL:-0}"

log() { printf '==> %s\n' "$*"; }

rm -rf "$STAGING"
mkdir -p "$STAGING"

export GA4GH_INFRA_REF FERRUM_VERSION="${VERSION}"

log "Building Compose images (ga4gh-infra @ ${GA4GH_INFRA_REF})..."
# shellcheck disable=SC2086
${COMPOSE_CMD} -f "$COMPOSE_FILE" build ferrum-gateway ferrum-ui ferrum-init

OFFLINE_OK=0
log "Exporting offline image bundle..."
chmod +x scripts/export_offline_bundle.sh
if ./scripts/export_offline_bundle.sh \
  --output-dir "${STAGING}/offline-bundle" \
  --compose-file "$COMPOSE_FILE" \
  --version "$VERSION" \
  --gateway-image "ferrum-gateway:${VERSION}"; then
  OFFLINE_OK=1
else
  echo "WARNING: offline bundle export failed" >&2
  if [[ "${OPTIONAL_OFFLINE}" != "1" ]]; then
    exit 1
  fi
  echo "FERRUM_OFFLINE_BUNDLE_OPTIONAL=1 — continuing without offline tarball" >&2
fi

log "Packaging Helm chart (version ${VERSION})..."
helm package deploy/helm \
  --version "${VERSION#v}" \
  --app-version "${VERSION}" \
  -d "$STAGING"
mv "${STAGING}/ferrum-${VERSION#v}.tgz" "${STAGING}/ferrum-helm-${VERSION}.tgz"
cp "${STAGING}/ferrum-helm-${VERSION}.tgz" "$ROOT/"

if [[ "${OFFLINE_OK}" == "1" ]]; then
  BUNDLE_DIR="${STAGING}/ferrum-offline-${VERSION}"
  mkdir -p "${BUNDLE_DIR}/deploy" "${BUNDLE_DIR}/helm" "${BUNDLE_DIR}/scripts"
  cp -R "${STAGING}/offline-bundle/." "${BUNDLE_DIR}/"
  cp VERSIONS.lock install.sh import.sh "${BUNDLE_DIR}/"
  cp scripts/import_offline_bundle.sh "${BUNDLE_DIR}/scripts/"
  cp deploy/docker-compose.yml deploy/.env.example deploy/nginx-demo.conf "${BUNDLE_DIR}/deploy/"
  mkdir -p "${BUNDLE_DIR}/deploy/object-store"
  cp deploy/object-store/seaweed-s3.json "${BUNDLE_DIR}/deploy/object-store/"
  {
    echo "FERRUM_VERSION=${VERSION}"
    grep -v '^FERRUM_VERSION=' deploy/.env.example | grep -v '^#' | grep -v '^$' || true
  } > "${BUNDLE_DIR}/.env"
  cp deploy/helm/values.yaml.example "${BUNDLE_DIR}/helm/"
  cp "${STAGING}/ferrum-helm-${VERSION}.tgz" "${BUNDLE_DIR}/helm/"
  cp docs/kubernetes-deployment.md "${BUNDLE_DIR}/docs-kubernetes-deployment.md" 2>/dev/null || true
  cp docs/TESTED-S3-BACKENDS.md "${BUNDLE_DIR}/docs-TESTED-S3-BACKENDS.md" 2>/dev/null || true
  tar -czf "ferrum-offline-${VERSION}.tar.gz" -C "$STAGING" "ferrum-offline-${VERSION}"
  log "Created ferrum-offline-${VERSION}.tar.gz"
else
  echo "offline_bundle=skipped" > "${STAGING}/offline-bundle-status.txt"
fi

log "Created ferrum-helm-${VERSION}.tgz"
