#!/usr/bin/env bash
# Print the compose invocation prefix (no -f / env files).
# Prefers `docker compose` plugin, then standalone `docker-compose`.
set -euo pipefail
export PATH="${HOME}/bin:/usr/local/bin:/opt/homebrew/bin:/Applications/Docker.app/Contents/Resources/bin:${PATH}"
if docker compose version >/dev/null 2>&1; then
  echo "docker compose"
  exit 0
fi
if [[ -x "${HOME}/.docker/cli-plugins/docker-compose" ]]; then
  echo "${HOME}/.docker/cli-plugins/docker-compose"
  exit 0
fi
if command -v docker-compose >/dev/null 2>&1; then
  echo "docker-compose"
  exit 0
fi
echo "compose-cmd: neither docker compose nor docker-compose found" >&2
exit 1
