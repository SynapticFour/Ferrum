#!/usr/bin/env sh
DEMO_DIR="$(dirname "$0")"
docker compose -f "$DEMO_DIR/docker-compose.demo.yml" down
echo "Ferrum demo stopped."
