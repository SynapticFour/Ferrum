#!/usr/bin/env sh
set -e

DEMO_DIR="$(dirname "$0")"
COMPOSE_FILE="$DEMO_DIR/docker-compose.demo.yml"

echo ""
echo "  🧬 Ferrum — GA4GH Bioinformatics Platform"
echo "  © 2025 Synaptic Four 🇩🇪"
echo ""
echo "  Starting demo stack..."
echo ""

# Check Docker is running
if ! docker info >/dev/null 2>&1; then
  echo "  ❌ Docker is not running. Please start Docker Desktop first."
  exit 1
fi

# Pull latest images
echo "  Pulling latest images..."
docker compose -f "$COMPOSE_FILE" pull

# Start stack
docker compose -f "$COMPOSE_FILE" up -d

echo ""
echo "  ✅ Ferrum is running!"
echo ""
echo "  UI:      http://localhost:3000"
echo "  API:     http://localhost:8080"
echo "  Docs:    http://localhost:8080/swagger-ui"
echo ""
echo "  To stop: ferrum demo stop"
echo ""
