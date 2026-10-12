# MinIO client (`mc`) mirror for offline / CDN-outage builds

`deploy/Dockerfile.init` installs `mc` via `deploy/scripts/fetch-mc.sh`.

Primary URL: pinned GitHub release `RELEASE.2025-08-13T08-35-41Z` (checksum verified).
`https://dl.min.io/client/mc/release/...` currently returns **HTTP 410**.

To vendor offline:

```bash
# amd64
curl -fsSL -o mc.linux-amd64.RELEASE.2025-08-13T08-35-41Z \
  https://github.com/minio/mc/releases/download/RELEASE.2025-08-13T08-35-41Z/mc.linux-amd64.RELEASE.2025-08-13T08-35-41Z
# sha256 01f866e9c5f9b87c2b09116fa5d7c06695b106242d829a8bb32990c00312e891

# arm64
curl -fsSL -o mc.linux-arm64.RELEASE.2025-08-13T08-35-41Z \
  https://github.com/minio/mc/releases/download/RELEASE.2025-08-13T08-35-41Z/mc.linux-arm64.RELEASE.2025-08-13T08-35-41Z
# sha256 14c8c9616cfce4636add161304353244e8de383b2e2752c0e9dad01d4c27c12c
```

Place the files in this directory (gitignored binaries are fine; do not commit large blobs unless the release process requires it).
