#!/usr/bin/env sh
# Retry docker compose build — mitigates transient Docker Hub 504 / registry timeouts in CI.
set -eu
max="${DOCKER_BUILD_RETRIES:-4}"
delay="${DOCKER_BUILD_RETRY_DELAY_SEC:-15}"
n=1
while true; do
  if docker compose "$@"; then
    exit 0
  fi
  if [ "$n" -ge "$max" ]; then
    echo "docker compose failed after $n attempt(s)"
    exit 1
  fi
  echo "docker compose build failed (attempt $n/$max), retrying in ${delay}s..."
  sleep "$delay"
  n=$((n + 1))
done
