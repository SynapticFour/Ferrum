# Field operations & resilience (Phase 6)

Operator guide for backup, integrity checks, solar power modes, and Pi deployment hygiene.

Related: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md), [FIELD-REGULATORY.md](FIELD-REGULATORY.md).

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

Sources are AC, battery, or unknown. UPS is not a detected source.

`[power] enabled` defaults to on only for a Linux build (`cfg!(target_os = "linux")`). On other platforms the default is off unless the config sets `enabled = true`. With monitoring off, the mode stays high performance even if a battery reading would otherwise be critical.

When monitoring is enabled, AC stays high performance before the percent thresholds. If the source is not AC and the percent is at or below 50 (default), concurrency is capped and background work pauses. At or below 10 (default), new requests are refused, a JSON note is written to `~/.ferrum/CHECKPOINT`, and the process exits. Startup does not read that note. This is not recovery from power loss.

| Mode | Trigger | HTTP behaviour |
|------|---------|----------------|
| HighPerformance | Monitoring off, or source AC, or battery above the low threshold | Normal |
| LowPower | Monitoring on, source not AC, battery at or below 50% (default) | Max 4 concurrent requests; background checksum/index paused |
| Emergency | Monitoring on, source not AC, battery at or below 10% (default) | **503** on new requests; JSON note, then exit. The note is not read at startup. |

Override for testing: `FERRUM_POWER_MODE=low_power|emergency|high_performance`. That override does not detect a UPS and does not restore from the note.

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
