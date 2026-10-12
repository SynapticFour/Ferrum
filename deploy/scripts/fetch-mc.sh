#!/usr/bin/env bash
# Pin, verify, retry, and optionally fall back for the MinIO client (`mc`).
# Used by deploy/Dockerfile.init. dl.min.io currently returns HTTP 410; primary
# source is the pinned GitHub release asset.
set -euo pipefail

MC_RELEASE="${MC_RELEASE:-RELEASE.2025-08-13T08-35-41Z}"
ARCH="${1:-amd64}"
DEST="${2:-/usr/local/bin/mc}"
MIRROR_DIR="${MC_MIRROR_DIR:-}"

case "$ARCH" in
  amd64|x86_64)
    ASSET="mc.linux-amd64.${MC_RELEASE}"
    EXPECT_SHA256="01f866e9c5f9b87c2b09116fa5d7c06695b106242d829a8bb32990c00312e891"
    ;;
  arm64|aarch64)
    ASSET="mc.linux-arm64.${MC_RELEASE}"
    EXPECT_SHA256="14c8c9616cfce4636add161304353244e8de383b2e2752c0e9dad01d4c27c12c"
    ;;
  *)
    echo "fetch-mc: unsupported arch '$ARCH' (want amd64 or arm64)" >&2
    exit 2
    ;;
esac

PRIMARY_URL="https://github.com/minio/mc/releases/download/${MC_RELEASE}/${ASSET}"
# Historical CDN (often 410 Gone as of 2026-10); kept as a secondary probe only.
LEGACY_URL="https://dl.min.io/client/mc/release/linux-${ARCH}/mc"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

install_from_path() {
  local src="$1"
  local got
  got="$(sha256_file "$src")"
  if [ "$got" != "$EXPECT_SHA256" ]; then
    echo "fetch-mc: checksum mismatch for $src (got $got want $EXPECT_SHA256)" >&2
    return 1
  fi
  install -m 0755 "$src" "$DEST"
  echo "fetch-mc: installed $DEST from $src (sha256 ok, ${MC_RELEASE})"
}

download_with_retries() {
  local url="$1"
  local out="$2"
  local attempt
  for attempt in 1 2 3; do
    if command -v wget >/dev/null 2>&1; then
      if wget -q -O "$out" "$url"; then
        return 0
      fi
    elif command -v curl >/dev/null 2>&1; then
      if curl -fsSL -o "$out" "$url"; then
        return 0
      fi
    else
      echo "fetch-mc: need wget or curl" >&2
      return 1
    fi
    echo "fetch-mc: attempt $attempt failed for $url" >&2
    sleep $((attempt * 2))
  done
  return 1
}

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# 1) Repository mirror (vendored offline binary)
if [ -n "$MIRROR_DIR" ] && [ -f "$MIRROR_DIR/$ASSET" ]; then
  if install_from_path "$MIRROR_DIR/$ASSET"; then
    exit 0
  fi
fi
if [ -f "/third_party/mc/$ASSET" ]; then
  if install_from_path "/third_party/mc/$ASSET"; then
    exit 0
  fi
fi

# 2) Pinned GitHub release
if download_with_retries "$PRIMARY_URL" "$TMP"; then
  if install_from_path "$TMP"; then
    exit 0
  fi
fi

# 3) Legacy CDN (likely 410)
if download_with_retries "$LEGACY_URL" "$TMP"; then
  if install_from_path "$TMP"; then
    exit 0
  fi
fi

cat >&2 <<EOF
fetch-mc: FAILED to obtain MinIO client mc (${MC_RELEASE}, arch=${ARCH}).
  Tried: mirror dir, GitHub ${PRIMARY_URL}, legacy ${LEGACY_URL}.
  To proceed offline, place the verified binary at third_party/mc/${ASSET}
  (sha256 ${EXPECT_SHA256}) and rebuild, or set MC_MIRROR_DIR.
  Init will skip bucket create if mc is absent (see init-demo.sh).
EOF
exit 1
