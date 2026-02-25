#!/usr/bin/env sh
set -e

REPO="SynapticFour/Ferrum"
BIN_NAME="ferrum-gateway"
INSTALL_DIR="$HOME/.ferrum/bin"

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

# Get latest release tag from GitHub API
echo "Fetching latest Ferrum release..."
LATEST=$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Error: Could not determine latest release."
  echo "Check https://github.com/$REPO/releases"
  exit 1
fi

echo "Latest release: $LATEST"

# Download URL
URL="https://github.com/$REPO/releases/download/$LATEST/$TARGET.tar.gz"

echo "Downloading $TARGET..."
curl -sSfL "$URL" -o /tmp/ferrum-download.tar.gz

# Extract
mkdir -p "$INSTALL_DIR"
tar -xzf /tmp/ferrum-download.tar.gz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/$BIN_NAME"
rm /tmp/ferrum-download.tar.gz

echo ""
echo "Ferrum installed to $INSTALL_DIR/$BIN_NAME"
echo ""
echo "Add Ferrum to your PATH by adding this to your ~/.zshrc or ~/.bashrc:"
echo ""
echo '  export PATH="$HOME/.ferrum/bin:$PATH"'
echo ""
echo "Then run: ferrum-gateway --version"
