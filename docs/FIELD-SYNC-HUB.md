# Field sync — hub conflict policy

When an Edge node pushes objects to a central hub via `ferrum sync push`, the hub may already hold data for the same biological sample or DRS alias. Ferrum Edge **does not auto-merge** conflicting records.

## Hub-side policy (recommended)

| Scenario | HTTP | Hub behaviour | Edge operator action |
|----------|------|---------------|----------------------|
| Duplicate `sample_id` / alias | **409 Conflict** | Reject upload; return JSON `code: conflict` | Review hub message; rename sample in ferrum-meta or contact hub admin |
| Same content, new version | **409** or **201** with suffix | Hub assigns version suffix (`sample-v2`) | Accept new DRS id from hub response |
| Idempotent retry | **200** / job dedup | Hub honours `client_request_id` (`ferrum-sync-{queue-id}`) | Safe to re-run `sync push` after transient failure |

## Edge behaviour

- On **409**, queue item → `failed` with message referencing this document.
- `bytes_sent` and `resume_token` retained for chunked retry when hub supports resume.
- Successful pushes append `sync_push_completed` to [DATA-RESIDENCY-AUDIT.md](DATA-RESIDENCY-AUDIT.md).

## Consent / DUO before enqueue

Configure selective sync in `config.toml`:

```toml
[sync]
require_metadata_ref = true
allowed_duo_codes = ["DUO:0000006", "DUO:0000007"]
allowed_consent_types = ["H3AFRICA_BROAD"]
```

Objects failing policy are skipped at `ferrum sync enqueue` with a warning (not queued).

## Related

- [FIELD-SYNC-QUEUE.md](FIELD-SYNC-QUEUE.md)
- [FIELD-AUTH-OFFLINE.md](FIELD-AUTH-OFFLINE.md) — `ferrum:sync_operator` role
- [DECISIONS.md](../DECISIONS.md) ADR-019, ADR-021
