#!/usr/bin/env sh
set -e

REPO="SynapticFour/Ferrum"
BIN_NAME="ferrum-gateway"
INSTALL_DIR="$HOME/.ferrum/bin"
OFFLINE=0

for arg in "$@"; do
  case "$arg" in
    --offline)
      OFFLINE=1
      ;;
  esac
done

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
  if [ -x "./scripts/build-laptop-native.sh" ]; then
    echo "[ferrum] Building optimized Laptop Mode binary for this machine..."
    ./scripts/build-laptop-native.sh --install
    exit 0
  fi
  LAPTOP_BIN=""
  for candidate in \
    "./target/release-laptop/ferrum-gateway" \
    "./target/release/ferrum-gateway" \
    "./target/"*"/release-laptop/ferrum-gateway"; do
    if [ -f "$candidate" ]; then
      LAPTOP_BIN="$candidate"
      break
    fi
  done
  if [ -n "$LAPTOP_BIN" ]; then
    mkdir -p "$INSTALL_DIR"
    cp "$LAPTOP_BIN" "$INSTALL_DIR/$BIN_NAME"
    ln -sf "$INSTALL_DIR/$BIN_NAME" "$INSTALL_DIR/ferrum"
    echo "Installed local build from $LAPTOP_BIN"
    echo "Run: ferrum demo start --offline"
    exit 0
  fi
  if [ -f "./ferrum-offline-bundle.tar.gz" ]; then
    echo "Import offline bundle with: ./scripts/import_offline_bundle.sh ./ferrum-offline-bundle.tar.gz"
    exit 0
  fi
  echo "Error: offline install requires ./scripts/build-laptop-native.sh, a pre-built binary under ./target/,"
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
echo ""
