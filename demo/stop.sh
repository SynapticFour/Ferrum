#!/usr/bin/env sh
set -e

DEMO_DIR="$(dirname "$0")"
COMPOSE_FILE="$DEMO_DIR/docker-compose.demo.yml"

if [ -n "${FERRUM_REPO:-}" ] && [ -f "${FERRUM_REPO}/deploy/docker-compose.yml" ]; then
  COMPOSE_FILE="${FERRUM_REPO}/deploy/docker-compose.yml"
elif [ -f "$DEMO_DIR/../deploy/docker-compose.yml" ]; then
  COMPOSE_FILE="$DEMO_DIR/../deploy/docker-compose.yml"
fi

docker compose -f "$COMPOSE_FILE" down
echo "Ferrum demo stopped."
