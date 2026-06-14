#!/usr/bin/env sh
set -e

DEMO_DIR="$(dirname "$0")"
COMPOSE_FILE="$DEMO_DIR/docker-compose.demo.yml"

# Prefer full repo stack when cloned (Postgres + MinIO + Keycloak + init seeding).
REPO_ROOT=""
if [ -n "${FERRUM_REPO:-}" ] && [ -f "${FERRUM_REPO}/deploy/docker-compose.yml" ]; then
  REPO_ROOT="${FERRUM_REPO}"
elif [ -f "$DEMO_DIR/../deploy/docker-compose.yml" ]; then
  REPO_ROOT="$(cd "$DEMO_DIR/.." && pwd)"
fi

if [ -n "$REPO_ROOT" ]; then
  COMPOSE_FILE="$REPO_ROOT/deploy/docker-compose.yml"
fi

echo ""
echo "  🧬 Ferrum — GA4GH Bioinformatics Platform"
echo "  © Synaptic Four"
echo ""
echo "  Starting demo stack..."
echo "  Compose: $COMPOSE_FILE"
echo ""

if ! docker info >/dev/null 2>&1; then
  echo "  ❌ Docker is not running. Start the Docker daemon, or use: ferrum demo start --offline"
  exit 1
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "  ❌ Docker Compose v2 plugin not found. Install docker-compose-plugin (Ubuntu: apt install docker-compose-plugin)."
  exit 1
fi

echo "  Pulling images..."
docker compose -f "$COMPOSE_FILE" pull || true

if [ -n "$REPO_ROOT" ]; then
  echo "  Building full demo stack..."
  (cd "$REPO_ROOT" && docker compose -f deploy/docker-compose.yml up -d --build)
else
  docker compose -f "$COMPOSE_FILE" up -d
fi

echo ""
echo "  ✅ Ferrum is running!"
echo ""
echo "  UI:      http://localhost:8082"
echo "  API:     http://localhost:8080"
echo ""
echo "  To stop: ferrum demo stop"
echo ""
