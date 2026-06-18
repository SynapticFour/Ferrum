#!/usr/bin/env bash
# Build an optimized single-binary Ferrum gateway for Edge mode on this machine.
#
# Auto-detects OS/CPU and applies native optimizations when safe.
#
# Usage:
#   ./scripts/build-edge-native.sh              # build only
#   ./scripts/build-edge-native.sh --install    # install to ~/.ferrum/bin
#   ./scripts/build-edge-native.sh --no-native-cpu   # generic CPU (portable on same arch)
#   ./scripts/build-edge-native.sh --target aarch64-unknown-linux-gnu  # cross hint (no native CPU)
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

INSTALL=0
NATIVE_CPU=1
CROSS_TARGET=""
PROFILE="${CARGO_PROFILE:-release-edge}"
INSTALL_DIR="${FERRUM_INSTALL_DIR:-$HOME/.ferrum/bin}"

while [ $# -gt 0 ]; do
  case "$1" in
    --install) INSTALL=1; shift ;;
    --no-native-cpu) NATIVE_CPU=0; shift ;;
    --target) CROSS_TARGET="$2"; shift 2 ;;
    --profile)
      PROFILE="$2"
      if [ "$PROFILE" = "release-laptop" ]; then
        echo "[ferrum] Warning: profile release-laptop is deprecated; use release-edge" >&2
        PROFILE="release-edge"
      fi
      shift 2
      ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64) RUST_ARCH="x86_64" ;;
  aarch64|arm64) RUST_ARCH="aarch64" ;;
  armv7l|armv6l) RUST_ARCH="arm" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

detect_target_triple() {
  if [ -n "$CROSS_TARGET" ]; then
    echo "$CROSS_TARGET"
    return
  fi
  case "$OS" in
    Linux)
      if [ "$RUST_ARCH" = "x86_64" ]; then
        echo "x86_64-unknown-linux-gnu"
      elif [ "$RUST_ARCH" = "aarch64" ]; then
        echo "aarch64-unknown-linux-gnu"
      else
        echo "armv7-unknown-linux-gnueabihf"
      fi
      ;;
    Darwin)
      if [ "$RUST_ARCH" = "aarch64" ]; then
        echo "aarch64-apple-darwin"
      else
        echo "x86_64-apple-darwin"
      fi
      ;;
    *)
      echo "Unsupported OS for auto-detect: $OS (use --target)" >&2
      exit 1
      ;;
  esac
}

TARGET="$(detect_target_triple)"
HOST_TRIPLE="$(rustc -vV 2>/dev/null | awk '/host:/ {print $2}' || true)"

echo "[ferrum] Edge mode native build"
echo "[ferrum]   OS:      $OS"
echo "[ferrum]   Arch:    $ARCH"
echo "[ferrum]   Target:  $TARGET"
echo "[ferrum]   Profile: $PROFILE"

USE_TARGET_FLAG=1
if [ -z "$CROSS_TARGET" ] && [ -n "$HOST_TRIPLE" ] && [ "$TARGET" = "$HOST_TRIPLE" ]; then
  USE_TARGET_FLAG=0
fi

export RUSTFLAGS="${RUSTFLAGS:-}"
if [ "$NATIVE_CPU" -eq 1 ] && [ "$USE_TARGET_FLAG" -eq 0 ]; then
  export RUSTFLAGS="${RUSTFLAGS} -C target-cpu=native"
  echo "[ferrum]   CPU:     native (-C target-cpu=native)"
elif [ "$NATIVE_CPU" -eq 1 ] && [ -n "$CROSS_TARGET" ]; then
  echo "[ferrum]   CPU:     generic (cross-compiling to $TARGET)"
else
  echo "[ferrum]   CPU:     generic (portable for $TARGET)"
fi

if [ "$USE_TARGET_FLAG" -eq 1 ]; then
  if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
      echo "[ferrum] Installing Rust target $TARGET..."
      rustup target add "$TARGET"
    fi
  elif [ "$TARGET" != "$HOST_TRIPLE" ]; then
    echo "Error: cross-target $TARGET requires rustup (not found in PATH)." >&2
    exit 1
  fi
fi

echo "[ferrum] Building ferrum-gateway (edge feature set)..."
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
if [ "$USE_TARGET_FLAG" -eq 1 ]; then
  cargo build --profile "$PROFILE" -p ferrum-gateway \
    --target "$TARGET" \
    --no-default-features --features edge
  BIN="$TARGET_DIR/$TARGET/$PROFILE/ferrum-gateway"
else
  cargo build --profile "$PROFILE" -p ferrum-gateway \
    --no-default-features --features edge
  BIN="$TARGET_DIR/$PROFILE/ferrum-gateway"
fi
if [ ! -x "$BIN" ]; then
  echo "Build failed: $BIN not found" >&2
  exit 1
fi

SIZE="$(wc -c <"$BIN" | tr -d ' ')"
echo "[ferrum] Built: $BIN ($SIZE bytes)"

if [ "$INSTALL" -eq 1 ]; then
  mkdir -p "$INSTALL_DIR"
  cp "$BIN" "$INSTALL_DIR/ferrum-gateway"
  ln -sf "$INSTALL_DIR/ferrum-gateway" "$INSTALL_DIR/ferrum"
  echo "[ferrum] Installed to $INSTALL_DIR/ferrum-gateway"
  echo "[ferrum] Run: ferrum demo start --edge"
fi
