#!/usr/bin/env bash
# Thin wrapper for air-gap import (see scripts/import_offline_bundle.sh).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
exec "${ROOT}/scripts/import_offline_bundle.sh" "$@"
