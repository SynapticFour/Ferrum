# Installation

This guide covers prerequisites, quick demo, building from source, single-node production, Ansible-based HPC deployment, Kubernetes/Helm, configuration reference, upgrading, uninstall, and troubleshooting.

---

## Prerequisites

| Requirement | Minimum | Recommended | Notes |
|-------------|---------|-------------|--------|
| Rust | 1.75+ | 1.83+ | For building from source |
| Docker | 20.10+ | 24+ | For demo stack |
| PostgreSQL | 14 | 16 | For production |
| MinIO or S3 | — | — | S3-compatible storage |
| Keycloak (optional) | 22 | 23 | OIDC / Passports |

---

## License & Usage

Ferrum is licensed under the Business Source License 1.1 (BUSL-1.1).
Free to use for:

- Academic and university research
- Non-commercial scientific projects
- Personal learning and evaluation
- Institutions running Ferrum for internal research

Commercial license required for:

- Offering Ferrum as a hosted/managed service to third parties
- Commercial genomics pipelines or products built on Ferrum

For commercial licensing inquiries, open an issue or discussion at:
https://github.com/SynapticFour/Ferrum

After four years from each release, that version becomes Apache-2.0.

---

## Quick demo (macOS / Linux)

```bash
# Install ferrum CLI
curl -sSf https://raw.githubusercontent.com/SynapticFour/Ferrum/main/install.sh | sh

# Start complete demo stack (Docker required)
ferrum demo start

# Verify all services are healthy
ferrum status
```

The demo stack includes: **ferrum-gateway**, **PostgreSQL 16**, **MinIO**, **Keycloak** (realm + test users), **ferrum-ui**, and **nginx**. Pre-seeded DRS objects (public genomic URLs) and test users (e.g. `alice`/`bob`) are created by the init container. Access the UI at the URL printed by `ferrum status` (e.g. http://localhost:8082).

---

## Building from source

```bash
git clone https://github.com/SynapticFour/Ferrum
cd Ferrum
cargo build --release
# Binaries in target/release/ferrum-gateway
```

On **Apple M4 (aarch64)**:

```bash
cargo build --release --target aarch64-apple-darwin
```

---

## Single-node production install

### a) Install PostgreSQL 16 and run migrations

```bash
# Debian/Ubuntu
sudo apt install postgresql-16

sudo -u postgres createuser -P ferrum
sudo -u postgres createdb -O ferrum ferrum
```

Migrations run automatically on first gateway start if `run_migrations = true` in config.

### b) Install MinIO or configure S3

Use MinIO (`minio server /data`) or any S3-compatible endpoint. Create a bucket (e.g. `ferrum`) and note endpoint, access key, and secret.

### c) Download Ferrum binary

Download from [GitHub Releases](https://github.com/SynapticFour/Ferrum/releases) for your platform (e.g. `ferrum-gateway-x86_64-unknown-linux-musl.tar.gz`). Extract to `/usr/local/bin/ferrum-gateway`.

### d) Create `/etc/ferrum/config.toml`

```toml
bind = "0.0.0.0:8080"

[database]
url = "postgres://ferrum:YOUR_PASSWORD@localhost:5432/ferrum"
run_migrations = true

[storage]
backend = "s3"
s3_endpoint = "https://minio.example.com"
s3_region = "us-east-1"
s3_bucket = "ferrum"
s3_access_key_id = "YOUR_ACCESS_KEY"
s3_secret_access_key = "YOUR_SECRET_KEY"

[auth]
require_auth = true
jwks_url = "https://auth.example.com/realms/ferrum/protocol/openid-connect/certs"

[services]
enable_drs = true
enable_wes = true
enable_tes = true
enable_trs = true
enable_beacon = true
enable_passports = true
enable_crypt4gh = true
```

### e) Create systemd service

```ini
# /etc/systemd/system/ferrum-gateway.service
[Unit]
Description=Ferrum GA4GH Gateway
After=network.target postgresql.service

[Service]
Type=simple
User=ferrum
Group=ferrum
ExecStart=/usr/local/bin/ferrum-gateway
Environment="FERRUM_CONFIG=/etc/ferrum/config.toml"
WorkingDirectory=/var/lib/ferrum
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### f) Generate Crypt4GH node keypair

```bash
ferrum keys generate
# Keys in /etc/ferrum/keys/ or path from config
```

### g) Start and enable service

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ferrum-gateway
```

### h) Configure nginx reverse proxy

```nginx
upstream ferrum {
    server 127.0.0.1:8080;
}
server {
    listen 443 ssl http2;
    server_name ferrum.example.com;
    ssl_certificate     /etc/ssl/certs/ferrum.pem;
    ssl_certificate_key /etc/ssl/private/ferrum-key.pem;
    location / {
        proxy_pass http://ferrum;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        client_max_body_size 100M;
    }
}
```

### i) Verify

```bash
ferrum status
curl -s https://your-host/health
```

---

## Distributed HPC (Ansible)

```bash
cd deploy/ansible
cp inventory/hpc.yml.example inventory/hpc.yml
# Edit inventory with your nodes
ansible-playbook playbooks/install-ferrum.yml -i inventory/hpc.yml
```

**Inventory structure:**

- **head_node** — Runs gateway, optionally PostgreSQL.
- **compute_nodes** — SLURM/LSF workers; WES/TES submit jobs here.
- **storage_nodes** — NFS or MinIO; Ferrum data and DRS objects.

Use **vault** for secrets (`vault_ferrum_db_password`, etc.). SLURM integration is configured via WES/TES executor settings (e.g. `workflow_engine_params` and Ferrum config).

---

## Kubernetes / Helm

```bash
helm repo add ferrum https://github.com/SynapticFour/Ferrum
helm repo update

# Install with external PostgreSQL and S3
helm install ferrum ferrum/ferrum \
  --namespace ferrum \
  --create-namespace \
  --set config.database.url="postgres://user:pass@postgres.example.com:5432/ferrum" \
  --set config.storage.s3_endpoint=https://minio.example.com \
  --set config.storage.s3_bucket=ferrum \
  --values values-production.yaml
```

**values-production.yaml** typically sets:

- `replicaCount`, `image.repository`, `image.tag`
- `existingSecret` for DB URL and S3 credentials
- `config.auth.jwks_url`, `config.auth.require_auth`
- `ingress.enabled`, `ingress.hosts`, `ingress.tls`
- `resources.limits` / `resources.requests`

---

## Configuration reference

| Section | Option | Type | Default | Description |
|---------|--------|------|---------|--------------|
| (root) | `bind` | string | `0.0.0.0:8080` | Listen address |
| `[database]` | `url` | string | — | PostgreSQL URL (overrides driver/params) |
| | `run_migrations` | bool | true | Run migrations on startup |
| | `max_connections` | u32 | 10 | Pool size |
| `[storage]` | `backend` | string | `local` | `local` or `s3` |
| | `base_path` | string | — | For local backend |
| | `s3_endpoint` | string | — | S3/MinIO endpoint |
| | `s3_region` | string | — | AWS region |
| | `s3_bucket` | string | — | Bucket name |
| | `s3_access_key_id` | string | — | Access key |
| | `s3_secret_access_key` | string | — | Secret key |
| `[auth]` | `require_auth` | bool | false | Require JWT/Passport |
| | `jwks_url` | string | — | JWKS for token validation |
| | `issuer` | string | — | Expected JWT issuer |
| `[services]` | `enable_drs` | bool | true | Enable DRS |
| | `enable_wes` | bool | true | Enable WES |
| | `enable_tes` | bool | true | Enable TES |
| | `enable_trs` | bool | true | Enable TRS |
| | `enable_beacon` | bool | true | Enable Beacon |
| | `enable_passports` | bool | true | Enable Passports |
| | `enable_crypt4gh` | bool | true | Enable Crypt4GH layer |
| | `enable_htsget` | bool | true | Enable htsget tickets and streams |
| `[encryption]` | `enabled` | bool | false | High-level encryption flag |
| `[logging]` | `level` | string | `info` | Log level |

### Environment-only tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `FERRUM_POSIX_IO_THREADS` | `32` | Size of the dedicated Rayon pool for blocking POSIX filesystem work (local DRS storage: put/delete/exists/size) and Crypt4GH on-disk key reads. Increase on HPC nodes with many concurrent local-object or decrypt workloads. |

---

## Upgrading

1. Download the new binary from [Releases](https://github.com/SynapticFour/Ferrum/releases).
2. Replace `/usr/local/bin/ferrum-gateway` (or your install path).
3. Run migrations if needed: `ferrum migrate` (or restart with `run_migrations = true`).
4. Restart the service: `sudo systemctl restart ferrum-gateway`.

Migrations are backward compatible within a major version. For major upgrades, check the release notes.

---

## Uninstall

**Remove service and binary (keep data):**

```bash
sudo systemctl stop ferrum-gateway
sudo systemctl disable ferrum-gateway
sudo rm /etc/systemd/system/ferrum-gateway.service
sudo rm /usr/local/bin/ferrum-gateway
sudo systemctl daemon-reload
```

**Full removal (including data):**

- Drop database: `DROP DATABASE ferrum;`
- Remove MinIO bucket or local data directory.
- Remove `/etc/ferrum/` and `/var/lib/ferrum/` if desired.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Can't connect to DB | Wrong URL, pg_hba.conf | Check `database.url`, allow host in `pg_hba.conf` |
| Crypt4GH decryption fails | Node key mismatch or wrong object | Ensure node keypair matches ingest key; verify object_id |
| WES workflows stuck in QUEUED | Executor config, SLURM connection | Check WES executor config, SLURM `squeue`, logs |
| 403 on DRS access | Passport expired or missing Visa | Refresh Passport; ensure Visa covers dataset/resource |
| 503 on service routes | Service disabled or state not built | Set `enable_*` in config and ensure gateway builds state |
| Health returns 503 | DB or storage unreachable | Check connectivity to PostgreSQL and S3/MinIO |

---

*[← Documentation index](README.md)*
