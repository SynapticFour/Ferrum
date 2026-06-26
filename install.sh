#!/usr/bin/env sh
set -e

REPO="SynapticFour/Ferrum"
BIN_NAME="ferrum-gateway"
INSTALL_DIR="$HOME/.ferrum/bin"
OFFLINE=0
COMPOSE_FILE="deploy/docker-compose.yml"

for arg in "$@"; do
  case "$arg" in
    --offline)
      OFFLINE=1
      ;;
  esac
done

compose_install() {
  if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
  fi

  if [ -z "${FERRUM_VERSION:-}" ]; then
    echo "ERROR: FERRUM_VERSION ist nicht gesetzt. Bitte .env konfigurieren."
    echo "       Beispiel: cp deploy/.env.example .env  und  FERRUM_VERSION=v0.2.0 setzen"
    exit 1
  fi

  export FERRUM_VERSION

  if ! command -v docker >/dev/null 2>&1; then
    echo "ERROR: Docker nicht gefunden."
    exit 1
  fi

  echo "[ferrum] Compose-Installation (Version ${FERRUM_VERSION})..."

  if [ "$OFFLINE" = "1" ]; then
    docker compose -f "$COMPOSE_FILE" up -d --pull never
  else
    docker compose -f "$COMPOSE_FILE" pull || true
    docker compose -f "$COMPOSE_FILE" up -d --build
  fi

  GATEWAY="${FERRUM_PUBLIC_BASE_URL:-http://localhost:${GATEWAY_PORT:-8080}}"
  echo "[ferrum] Warte auf ${GATEWAY}/health ..."
  i=0
  while [ "$i" -lt 24 ]; do
    if curl -sf "${GATEWAY}/health" >/dev/null 2>&1; then
      echo "[ferrum] Stack bereit: ${GATEWAY}/health"
      echo "[ferrum] UI: http://localhost:${UI_PORT:-8082}"
      return 0
    fi
    i=$((i + 1))
    sleep 5
  done

  echo "ERROR: Gateway nicht erreichbar unter ${GATEWAY}/health"
  docker compose -f "$COMPOSE_FILE" logs ferrum-gateway 2>&1 | tail -40 || true
  exit 1
}

if [ -f "$COMPOSE_FILE" ]; then
  compose_install
  exit 0
fi

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64)  TARGET="ferrum-gateway-x86_64-unknown-linux-musl" ;;
      aarch64) TARGET="ferrum-gateway-aarch64-unknown-linux-musl" ;;
      *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64)  TARGET="ferrum-gateway-aarch64-apple-darwin" ;;
      x86_64) TARGET="ferrum-gateway-x86_64-apple-darwin" ;;
      *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported operating system: $OS"
    exit 1
    ;;
esac

if [ "$OFFLINE" = "1" ]; then
  echo "[ferrum] Offline install mode."
  if [ -x "./scripts/build-edge-native.sh" ]; then
    echo "[ferrum] Building optimized Edge mode binary for this machine..."
    ./scripts/build-edge-native.sh --install
    exit 0
  fi
  EDGE_BIN=""
  for candidate in \
    "./target/release-edge/ferrum-gateway" \
    "./target/release-laptop/ferrum-gateway" \
    "./target/release/ferrum-gateway" \
    "./target/"*"/release-edge/ferrum-gateway"; do
    if [ -f "$candidate" ]; then
      EDGE_BIN="$candidate"
      break
    fi
  done
  if [ -n "$EDGE_BIN" ]; then
    mkdir -p "$INSTALL_DIR"
    cp "$EDGE_BIN" "$INSTALL_DIR/$BIN_NAME"
    ln -sf "$INSTALL_DIR/$BIN_NAME" "$INSTALL_DIR/ferrum"
    echo "Installed local build from $EDGE_BIN"
    echo "Run: ferrum demo start --edge"
    exit 0
  fi
  if [ -f "./ferrum-offline-bundle.tar.gz" ]; then
    echo "Import offline bundle with: ./import.sh"
    exit 0
  fi
  echo "Error: offline install requires ./scripts/build-edge-native.sh, a pre-built binary under ./target/,"
  echo "       or an offline bundle (see docs/deployment/OFFLINE-AIRGAP.md)."
  exit 1
fi

# Get latest release tag from GitHub API
echo "Fetching latest Ferrum release..."
if ! LATEST=$(curl -sSf --connect-timeout 5 "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'); then
  echo "Error: Could not reach GitHub API (offline?)."
  echo "Use: ./install.sh --offline after building locally or importing an offline bundle."
  exit 1
fi

if [ -z "$LATEST" ]; then
  echo "Error: Could not determine latest release."
  echo "Check https://github.com/$REPO/releases or use --offline"
  exit 1
fi

echo "Latest release: $LATEST"

URL="https://github.com/$REPO/releases/download/$LATEST/$TARGET.tar.gz"

echo "Downloading $TARGET..."
if ! curl -sSfL --connect-timeout 10 "$URL" -o /tmp/ferrum-download.tar.gz; then
  echo "Error: download failed. If you are offline, use ./install.sh --offline"
  exit 1
fi

mkdir -p "$INSTALL_DIR"
tar -xzf /tmp/ferrum-download.tar.gz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/$BIN_NAME"
ln -sf "$INSTALL_DIR/$BIN_NAME" "$INSTALL_DIR/ferrum"

DEMO_INSTALL_DIR="$HOME/.ferrum/demo"
mkdir -p "$DEMO_INSTALL_DIR"
if curl -sSfL --connect-timeout 5 "https://raw.githubusercontent.com/SynapticFour/Ferrum/main/demo/docker-compose.demo.yml" \
  -o "$DEMO_INSTALL_DIR/docker-compose.demo.yml" 2>/dev/null; then
  curl -sSfL --connect-timeout 5 "https://raw.githubusercontent.com/SynapticFour/Ferrum/main/demo/nginx-demo.conf" \
    -o "$DEMO_INSTALL_DIR/nginx-demo.conf" || true
  curl -sSfL --connect-timeout 5 "https://raw.githubusercontent.com/SynapticFour/Ferrum/main/demo/start.sh" \
    -o "$DEMO_INSTALL_DIR/start.sh" || true
  curl -sSfL --connect-timeout 5 "https://raw.githubusercontent.com/SynapticFour/Ferrum/main/demo/stop.sh" \
    -o "$DEMO_INSTALL_DIR/stop.sh" || true
  chmod +x "$DEMO_INSTALL_DIR/start.sh" "$DEMO_INSTALL_DIR/stop.sh" 2>/dev/null || true
else
  echo "[ferrum] Demo scripts not downloaded (offline). Use ferrum demo start --offline for Laptop Mode."
fi

rm -f /tmp/ferrum-download.tar.gz

echo ""
echo "Ferrum installed to $INSTALL_DIR/$BIN_NAME"
echo ""
echo "Add Ferrum to your PATH:"
echo '  export PATH="$HOME/.ferrum/bin:$PATH"'
echo ""
echo "Laptop / offline mode: ferrum demo start --offline"
echo "Docker demo (requires Docker): ferrum demo start"
echo "Full stack (MinIO, Keycloak, seeded data): git clone https://github.com/SynapticFour/Ferrum && cd Ferrum && make demo"
echo ""
