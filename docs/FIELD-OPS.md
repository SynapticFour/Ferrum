# Field operations & resilience (Phase 6)

Operator guide for backup, integrity checks, solar power modes, and Pi deployment hygiene.

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-MATURITY-PLAN.md](FIELD-MATURITY-PLAN.md), [FIELD-REGULATORY.md](FIELD-REGULATORY.md).

## SQLite backup & restore

Edge nodes store metadata in SQLite and objects on local disk. Back up before OS updates or device rotation:

```bash
# Gateway may stay running for create (SQLite WAL-safe read)
ferrum backup create --output ~/backups/ferrum-$(date +%Y%m%d).tar.gz

# Stop gateway first, then restore
sudo systemctl stop ferrum-gateway
ferrum backup restore --archive ~/backups/ferrum-20260619.tar.gz --force
sudo systemctl start ferrum-gateway
```

The archive contains `ferrum.db`, `manifest.json`, and (by default) the `objects/` tree.

## Integrity verification

DRS stores SHA-256 checksums after ingest. Verify local bytes against metadata:

```bash
ferrum backup verify
```

Enable automatic verification on gateway startup (refuses to start if mismatches are found):

```toml
[ops]
verify_checksums_on_startup = true
```

## Solar / battery power modes

When `[power] enabled = true` (default on Linux), Ferrum reads `/sys/class/power_supply/`:

| Mode | Trigger | HTTP behaviour |
|------|---------|----------------|
| HighPerformance | AC power | Normal |
| LowPower | Battery &lt; 50% | Max 4 concurrent requests; background checksum/index paused |
| Emergency | Battery &lt; 10% | **503** on new requests; checkpoint then exit |

Override for testing: `FERRUM_POWER_MODE=low_power|emergency|high_performance`.

HelixTest (optional): `helixtest --all --mode ferrum-africa --africa-profile power`

## systemd deployment

```bash
sudo useradd --system --home /var/lib/ferrum --shell /usr/sbin/nologin ferrum || true
sudo install -m 755 ferrum-gateway /usr/local/bin/
sudo mkdir -p /etc/ferrum /var/lib/ferrum
sudo cp deploy/systemd/ferrum-gateway.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now ferrum-gateway
```

Logs: `journalctl -u ferrum-gateway -f`. Limit journal size on Pi:

```bash
# /etc/systemd/journald.conf
SystemMaxUse=200M
```

Optional file logging + rotation: see [deploy/systemd/ferrum-gateway.logrotate](../deploy/systemd/ferrum-gateway.logrotate).

## ARM binary size & Crypt4GH throughput

- **Binary budget:** `ferrum-gateway` **release-edge** must stay **&lt; 50 MB** (CI `build-arm64` gate).
- **Crypt4GH on Pi 5:** target **&gt; 500 MB/s** encrypt with NEON (`release-edge-perf`). Measure:

```bash
cargo bench -p ferrum-crypt4gh --bench crypt_benchmark
```

CI compiles ARM64 benchmarks on every `main` push; field operators run the bench locally on hardware.

## Test gate

```bash
bash deploy/scripts/ci-field-ops-e2e.sh
```
