#!/usr/bin/env sh
# Install Ferrum gateway binary.
# Usage: curl -sSf https://ferrum-bio.sh/install | sh
# Or: curl -sSf https://raw.githubusercontent.com/OWNER/Ferrum/main/install.sh | sh

set -e

GITHUB_REPO="${GITHUB_REPO:-SynapticFour/Ferrum}"
INSTALL_DIR="${FERRUM_INSTALL_DIR:-$HOME/.ferrum/bin}"
VERSION="${FERRUM_VERSION:-latest}"

# Resolve latest tag
resolve_version() {
  if [ "$VERSION" = "latest" ]; then
    curl -sSf "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p'
  else
    echo "$VERSION"
  fi
}

# Detect platform
detect_platform() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64|amd64) echo "x86_64-apple-darwin" ;;
        *) echo "unsupported:$os-$arch" ;;
      esac ;;
    Linux)
      case "$arch" in
        x86_64|amd64) echo "x86_64-unknown-linux-musl" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-musl" ;;
        *) echo "unsupported:$os-$arch" ;;
      esac ;;
    *)
      echo "unsupported:$os-$arch" ;;
  esac
}

tag=$(resolve_version)
platform=$(detect_platform)

if [ -z "$tag" ]; then
  echo "Could not resolve release version." >&2
  exit 1
fi

if [ "${platform#unsupported}" != "$platform" ]; then
  echo "Unsupported platform: $platform" >&2
  exit 1
fi

asset_name="ferrum-gateway-${platform}"
url="https://github.com/${GITHUB_REPO}/releases/download/${tag}/${asset_name}.tar.gz"
echo "Installing Ferrum $tag for $platform to $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
curl -sSfL "$url" | tar -xzf - -C "$INSTALL_DIR" ferrum-gateway
chmod +x "$INSTALL_DIR/ferrum-gateway"
echo "Installed to $INSTALL_DIR/ferrum-gateway"
echo "Add to PATH: export PATH=\"$INSTALL_DIR:\$PATH\""
