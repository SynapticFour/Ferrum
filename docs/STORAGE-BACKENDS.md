# Storage backends

## Native Ferrum (`ferrum-storage`)

| Backend | Type | Notes |
|--------|------|--------|
| `LocalStorage` | POSIX path | Uses `ferrum_core::io::posix` for blocking I/O on a dedicated pool. |
| `S3Storage` | S3-compatible | In-memory `put_bytes` with multipart ≥ 5 MiB; **`S3Storage::put_file`** for large **on-disk** uploads (8 MiB / 64 MiB parts, parallel parts, abort on failure). |

## OpenDAL (optional)

Build with `ferrum-storage` feature **`opendal`**. Then use `OpenDalStorage::new(operator)` or `OpenDalStorage::from_local_dir("/data")`.

OpenDAL supports many services (S3, GCS, Azure Blob, OBS, iRODS, …). Configure the `opendal::Operator` per [OpenDAL docs](https://docs.rs/opendal).

**Note:** The current `OpenDalStorage::get` implementation reads the full object into memory; for multi‑TB objects prefer **`S3Storage`** streaming or extend OpenDAL with a streaming reader.

### Deployment hints

- **S3-compatible object store** (DKFZ / EMBL / Helmholtz-style): `S3Storage` or OpenDAL `s3`.
- **University HPC POSIX / NFS**: `LocalStorage` or OpenDAL `fs` with a mount root.
- **iRODS / multi-cloud**: OpenDAL with the matching `services-*` feature enabled in your own binary.
