#!/usr/bin/env bash
set -euo pipefail

SCENARIO="single-node"
REQUIRE_INTERNET="auto"
FAILED=0

pass() { echo "PASS: $1"; }
warn() { echo "WARN: $1"; }
fail() { echo "FAIL: $1"; FAILED=1; }

usage() {
  cat <<'EOF'
Usage:
  ./scripts/deployment_preflight.sh --scenario <demo|single-node|hpc|kubernetes|offline> [--require-internet true|false|auto]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scenario) SCENARIO="$2"; shift 2 ;;
    --require-internet) REQUIRE_INTERNET="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1"; usage; exit 1 ;;
  esac
done

check_cmd() {
  if command -v "$1" >/dev/null 2>&1; then pass "Command available: $1"; else fail "Command missing: $1"; fi
}

check_docker() {
  if docker info >/dev/null 2>&1; then pass "Docker daemon reachable"; else fail "Docker daemon not reachable"; fi
}

check_internet() {
  if curl -fsS --max-time 5 https://github.com >/dev/null 2>&1; then pass "Internet reachable"; else fail "Internet not reachable"; fi
}

get_ram_gb() {
  if [[ "$(uname -s)" == "Darwin" ]]; then
    local bytes
    bytes="$(sysctl -n hw.memsize)"
    echo $((bytes / 1024 / 1024 / 1024))
  else
    awk '/MemTotal/ {print int($2/1024/1024)}' /proc/meminfo
  fi
}

get_disk_gb() {
  df -Pk . | awk 'NR==2 {print int($4/1024/1024)}'
}

echo "Running preflight for scenario: ${SCENARIO}"
check_cmd curl

ram="$(get_ram_gb)"
disk="$(get_disk_gb)"
echo "Detected RAM: ${ram} GB"
echo "Free disk: ${disk} GB"

internet_needed="true"
if [[ "${REQUIRE_INTERNET}" == "true" ]]; then
  internet_needed="true"
elif [[ "${REQUIRE_INTERNET}" == "false" ]]; then
  internet_needed="false"
elif [[ "${SCENARIO}" == "offline" ]]; then
  internet_needed="false"
fi

if [[ "${internet_needed}" == "true" ]]; then check_internet; else warn "Internet check skipped"; fi

case "${SCENARIO}" in
  demo)
    check_cmd docker
    check_docker
    (( ram >= 8 )) && pass "RAM >= 8 GB" || fail "RAM < 8 GB"
    (( disk >= 20 )) && pass "Disk >= 20 GB" || fail "Disk < 20 GB"
    ;;
  single-node)
    (( ram >= 16 )) && pass "RAM >= 16 GB" || fail "RAM < 16 GB"
    (( disk >= 50 )) && pass "Disk >= 50 GB" || fail "Disk < 50 GB"
    ;;
  hpc)
    check_cmd ansible-playbook
    (( ram >= 32 )) && pass "RAM >= 32 GB" || fail "RAM < 32 GB"
    (( disk >= 100 )) && pass "Disk >= 100 GB" || fail "Disk < 100 GB"
    ;;
  kubernetes)
    check_cmd kubectl
    check_cmd helm
    if kubectl cluster-info >/dev/null 2>&1; then pass "kubectl can reach cluster"; else fail "kubectl cannot reach cluster"; fi
    ;;
  offline)
    check_cmd docker
    check_docker
    (( ram >= 16 )) && pass "RAM >= 16 GB" || fail "RAM < 16 GB"
    (( disk >= 80 )) && pass "Disk >= 80 GB" || fail "Disk < 80 GB"
    ;;
  *)
    fail "Unknown scenario: ${SCENARIO}"
    ;;
esac

if [[ "${FAILED}" -eq 1 ]]; then
  echo "Preflight finished with failures."
  exit 1
fi
echo "Preflight finished successfully."

