#!/usr/bin/env bash
# Source Ferrum/VERSIONS.lock into the current shell (export KEY=VALUE).
# Usage: source scripts/load-versions.sh
set -euo pipefail

_LOAD_VERSIONS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
_LOAD_VERSIONS_FILE="${VERSIONS_FILE:-$_LOAD_VERSIONS_ROOT/VERSIONS.lock}"

if [[ ! -f "$_LOAD_VERSIONS_FILE" ]]; then
  echo "load-versions: missing $_LOAD_VERSIONS_FILE" >&2
  return 1 2>/dev/null || exit 1
fi

while IFS= read -r line || [[ -n "$line" ]]; do
  line="${line%%#*}"
  line="$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
  [[ -z "$line" ]] && continue
  if [[ "$line" =~ ^([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
    export "${BASH_REMATCH[1]}=${BASH_REMATCH[2]}"
  fi
done < "$_LOAD_VERSIONS_FILE"
