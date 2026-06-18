#!/usr/bin/env bash
# Unified field-edge installer: Ferrum (data plane) + ga4gh-infra (identity plane).
#
# Targets Raspberry Pi 5 / ARM64 Linux field labs with intermittent connectivity.
# Installs native Edge-mode binaries — no Docker required for the slim stack.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/SynapticFour/Ferrum/main/scripts/install-field-edge.sh | bash
# Or from a clone:
#   ./scripts/install-field-edge.sh
#
# Environment:
#   FERRUM_REPO          GitHub repo (default: SynapticFour/Ferrum)
#   GA4GH_INFRA_REPO     GitHub repo (default: SynapticFour/ga4gh-infra)
#   FERRUM_INSTALL_DIR   default: ~/.ferrum/bin
#   GA4GH_INFRA_INSTALL_DIR  default: ~/.local/bin
#   SKIP_INFRA=1         Install Ferrum only
#   BUILD_FROM_SOURCE=1  Force local cargo build instead of release download

set -euo pipefail

FERRUM_REPO="${FERRUM_REPO:-SynapticFour/Ferrum}"
GA4GH_INFRA_REPO="${GA4GH_INFRA_REPO:-SynapticFour/ga4gh-infra}"
FERRUM_INSTALL_DIR="${FERRUM_INSTALL_DIR:-$HOME/.ferrum/bin}"
GA4GH_INFRA_INSTALL_DIR="${GA4GH_INFRA_INSTALL_DIR:-$HOME/.local/bin}"
SKIP_INFRA="${SKIP_INFRA:-0}"
BUILD_FROM_SOURCE="${BUILD_FROM_SOURCE:-0}"

log() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }

banner() {
  echo ""
  echo "╔══════════════════════════════════════════════════════════╗"
  echo "║  Ferrum Field Edge — unified installer                   ║"
  echo "║  Data plane (Ferrum Edge) + identity (ga4gh-infra)       ║"
  echo "╚══════════════════════════════════════════════════════════╝"
  echo ""
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) echo x86_64 ;;
    aarch64|arm64) echo aarch64 ;;
    *) warn "Architecture $arch — ARM64 (Pi 5) recommended for field edge"; echo "$arch" ;;
  esac
}

install_ferrum() {
  log "Installing Ferrum Edge binary"
  if [ "$BUILD_FROM_SOURCE" = "1" ] && [ -f "./scripts/build-edge-native.sh" ]; then
    FERRUM_INSTALL_DIR="$FERRUM_INSTALL_DIR" ./scripts/build-edge-native.sh --install
    return
  fi
  if [ -f "./install.sh" ]; then
    ./install.sh --offline
    return
  fi
  warn "No local Ferrum repo — downloading install.sh from GitHub"
  curl -fsSL "https://raw.githubusercontent.com/${FERRUM_REPO}/main/install.sh" | sh -s -- --offline
}

install_ga4gh_infra() {
  if [ "$SKIP_INFRA" = "1" ]; then
    warn "SKIP_INFRA=1 — skipping ga4gh-infra"
    return
  fi
  log "Installing ga4gh-infra (Africa / edge identity plane)"
  if [ -f "../ga4gh-infra/scripts/install.sh" ]; then
    GA4GH_INFRA_INSTALL_DIR="$GA4GH_INFRA_INSTALL_DIR" bash ../ga4gh-infra/scripts/install.sh
    return
  fi
  curl -fsSL "https://raw.githubusercontent.com/${GA4GH_INFRA_REPO}/main/scripts/install.sh" \
    | GA4GH_INFRA_INSTALL_DIR="$GA4GH_INFRA_INSTALL_DIR" sh
}

preflight() {
  log "Preflight checks"
  if [ -f "./scripts/deployment_preflight.sh" ]; then
    ./scripts/deployment_preflight.sh --scenario edge || \
      ./scripts/deployment_preflight.sh --scenario laptop || true
  fi
  if [ -r /proc/meminfo ]; then
    local ram_mb
    ram_mb="$(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo)"
    log "RAM: ${ram_mb} MB"
    if [ "$ram_mb" -lt 3800 ]; then
      warn "4 GB+ RAM recommended for Ferrum + ga4gh-infra co-deploy"
    fi
  fi
}

print_next_steps() {
  echo ""
  echo "Next steps:"
  echo "  1. export PATH=\"$FERRUM_INSTALL_DIR:$GA4GH_INFRA_INSTALL_DIR:\$PATH\""
  echo "  2. ga4gh-infra keygen --output-dir ~/.config/ga4gh-infra/secrets   # if infra installed"
  echo "  3. ga4gh-infra all-in-one --africa   # ports 8180–8190"
  echo "  4. ferrum demo start --edge            # port 8080, SQLite + local DRS"
  echo "  5. ferrum meta validate --input profiles/meta/fixtures/ferrum-core-minimal-submission.yaml"
  echo "  6. ferrum meta init --profile pathogen --output ~/collection.yaml  # field metadata wizard"
  echo "  7. ferrum auth account add --username alice --role collector --pin '****'  # shared device"
  echo ""
  echo "Auth offline: docs/FIELD-AUTH-OFFLINE.md"
  echo ""
}

main() {
  banner
  detect_arch >/dev/null
  preflight
  install_ferrum
  install_ga4gh_infra
  print_next_steps
}

main "$@"
