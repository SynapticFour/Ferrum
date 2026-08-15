#!/usr/bin/env bash
# Build release Docker images, offline bundle, and Helm chart for GitHub Release.
# Requires: VERSIONS.lock loaded (GA4GH_INFRA_REF), FERRUM_RELEASE_VERSION set (e.g. v0.2.0).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${FERRUM_RELEASE_VERSION:?set FERRUM_RELEASE_VERSION (e.g. v0.2.0)}"
GA4GH_INFRA_REF="${GA4GH_INFRA_REF:?load VERSIONS.lock first}"
STAGING="${RELEASE_STAGING_DIR:-release-staging}"
COMPOSE_FILE="${COMPOSE_FILE:-deploy/docker-compose.yml}"

log() { printf '==> %s\n' "$*"; }

rm -rf "$STAGING"
mkdir -p "$STAGING"

export GA4GH_INFRA_REF FERRUM_VERSION="${VERSION}"

log "Building Compose images (ga4gh-infra @ ${GA4GH_INFRA_REF})..."
docker compose -f "$COMPOSE_FILE" build ferrum-gateway ferrum-ui ferrum-init

log "Exporting offline image bundle..."
chmod +x scripts/export_offline_bundle.sh
./scripts/export_offline_bundle.sh \
  --output-dir "${STAGING}/offline-bundle" \
  --compose-file "$COMPOSE_FILE" \
  --version "$VERSION" \
  --gateway-image "ferrum-gateway:${VERSION}"

log "Packaging Helm chart (version ${VERSION})..."
helm package deploy/helm \
  --version "${VERSION#v}" \
  --app-version "${VERSION}" \
  -d "$STAGING"
mv "${STAGING}/ferrum-${VERSION#v}.tgz" "${STAGING}/ferrum-helm-${VERSION}.tgz"

BUNDLE_DIR="${STAGING}/ferrum-offline-${VERSION}"
mkdir -p "${BUNDLE_DIR}/deploy" "${BUNDLE_DIR}/helm" "${BUNDLE_DIR}/scripts"
cp -R "${STAGING}/offline-bundle/." "${BUNDLE_DIR}/"
cp VERSIONS.lock install.sh import.sh "${BUNDLE_DIR}/"
cp scripts/import_offline_bundle.sh "${BUNDLE_DIR}/scripts/"
cp deploy/docker-compose.yml deploy/.env.example deploy/nginx-demo.conf "${BUNDLE_DIR}/deploy/"
# Pre-fill FERRUM_VERSION for customer install.sh (passwords still from .env.example)
{
  echo "FERRUM_VERSION=${VERSION}"
  grep -v '^FERRUM_VERSION=' deploy/.env.example | grep -v '^#' | grep -v '^$' || true
} > "${BUNDLE_DIR}/.env"
cp deploy/helm/values.yaml.example "${BUNDLE_DIR}/helm/"
cp "${STAGING}/ferrum-helm-${VERSION}.tgz" "${BUNDLE_DIR}/helm/"
cp docs/kubernetes-deployment.md "${BUNDLE_DIR}/docs-kubernetes-deployment.md"

tar -czf "ferrum-offline-${VERSION}.tar.gz" -C "$STAGING" "ferrum-offline-${VERSION}"
cp "${STAGING}/ferrum-helm-${VERSION}.tgz" "$ROOT/"
log "Created ferrum-offline-${VERSION}.tar.gz"
log "Created ferrum-helm-${VERSION}.tgz"
