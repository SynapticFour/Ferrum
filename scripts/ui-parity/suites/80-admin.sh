#!/usr/bin/env bash
# Suite 80: Admin read-only (Settings federation + security panels).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

ui_log "suite 80-admin"

expect_2xx_soft "admin-federation-status" GET "/admin/federation/status"
expect_2xx_soft "admin-security-events" GET "/admin/security/events?limit=10"
expect_2xx_soft "wes-cache-stats" GET "/ga4gh/wes/v1/cache/stats"
