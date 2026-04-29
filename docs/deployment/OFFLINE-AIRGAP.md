# Offline / Air-Gapped Deployment

Ferrum supports isolated deployments by preparing artifacts on an online build host
and importing them into an offline target environment.

## Target Model

- **Online build host:** exports images/binaries/checksums.
- **Offline target:** imports artifacts and deploys with pinned versions.

## 1) Run preflight on target

```bash
./scripts/deployment_preflight.sh --scenario offline
```

## 2) Create offline bundle (online host)

```bash
./scripts/export_offline_bundle.sh \
  --output-dir ./offline-bundle \
  --gateway-image ferrum-gateway:latest \
  --compose-file deploy/docker-compose.yml
```

Artifacts:
- `images-*.tar.gz`
- `checksums.sha256`
- `manifest.txt`

## 3) Transfer bundle

Use approved media/process (e.g., signed internal transfer media).

## 4) Import bundle (offline target)

```bash
./scripts/import_offline_bundle.sh --bundle-dir ./offline-bundle
```

## 5) Deploy

For demo-like stack:

```bash
docker compose -f deploy/docker-compose.yml up -d
```

For production single-node:
- use pinned binary + config in `/etc/ferrum/config.toml`
- restart systemd service.

## Offline update and rollback

Update:
1. Build and sign new offline bundle online.
2. Import into staging target.
3. Run smoke tests (`/health`, core GA4GH route, auth if enabled).
4. Promote to production.

Rollback:
- keep previous bundle archived
- re-import previous bundle and restart services.

