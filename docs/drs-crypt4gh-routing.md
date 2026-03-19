# DRS and Crypt4GH routing

This note describes how to route encrypted DRS objects through the Crypt4GH re-encryption layer so clients can receive streams re-encrypted to their public key.

## S3 presigned URLs

When DRS objects are stored in S3-compatible storage (`storage_references.storage_backend` = `s3` or `minio`), enable the `s3_signed` feature and provide an S3 presigner in `AppState` so that `GET /objects/{id}/access/{access_id}` returns a presigned URL (with optional byte-range support when the client sends a `Range` header). Example at startup:

```rust
// With feature s3_signed:
let presigner = ferrum_drs::presign::create_presigner(
    bucket,
    region,
    Some(endpoint),  // or None for AWS
).await;
let drs_state = ferrum_drs::AppState {
    repo: Arc::new(ferrum_drs::DrsRepo::new(pool, hostname)),
    storage: Some(storage),
    s3_presigner: presigner,
    provenance_store: None,
    crypt4gh_key_dir: Some(std::path::PathBuf::from("/data/ferrum/keys")),
    crypt4gh_master_key_id: "node".to_string(),
    crypt4gh_decrypt_stream: true,
};
```

## Storage and encryption

- DRS objects can have `storage_references.is_encrypted = true`, meaning the blob at `storage_key` is encrypted with the service’s Crypt4GH master key.
- The standard DRS flow returns an **access URL** (e.g. presigned S3 URL or a gateway stream URL). The client then GETs that URL to download bytes.

## Option A: Stream URL on the gateway (recommended)

1. **Expose a stream endpoint** that serves object bytes for a DRS object: **`GET /ga4gh/drs/v1/objects/{object_id}/stream`** (implemented in Ferrum).  
   This handler should:
   - Resolve `object_id` (and aliases), load `storage_references` and backend/key.
   - For non-encrypted objects: stream from storage (or redirect to presigned URL).
   - For encrypted objects: stream from storage and **decrypt** with the master key, then return plaintext (or, if the route is wrapped with Crypt4GHLayer, return plaintext and let the layer re-encrypt).

2. **Wrap the stream route with Crypt4GHLayer** so that when the client sends `X-Crypt4GH-Public-Key` and has a valid passport/visa, the response body is re-encrypted to that key.  
   The layer expects the **inner** response to be the **plain** decrypted stream (it will re-encrypt it). So the stream handler for encrypted objects must:
   - Read from storage (encrypted blob),
   - Decrypt with master key (Crypt4GH decrypt),
   - Return that decrypted body to the layer,
   - The layer then re-encrypts to the client’s key.

3. **Point DRS access URLs at this stream** when you want “download via gateway with re-encryption”. For objects with `is_encrypted = true`, store an access method whose `access_url` is e.g. `https://{host}/ga4gh/drs/v1/objects/{object_id}/stream`. Then when a client calls `GET /objects/{id}/access/{access_id}`, they get that URL and GET it with the same auth and `X-Crypt4GH-Public-Key` header; the gateway stream endpoint + Crypt4GHLayer handle decryption and re-encryption.

### Plaintext stream (no client Crypt4GH)

When `encryption.crypt4gh_decrypt_stream` is `true` (default) and `encryption.crypt4gh_key_dir` points at the node key directory (`{crypt4gh_master_key_id}.sec`, default id `node`), **`GET .../objects/{id}/stream`** decrypts Crypt4GH at-rest bytes and streams **plaintext** to the client. Clients can use a normal HTTP client; no `X-Crypt4GH-Public-Key` is required. Set `crypt4gh_decrypt_stream = false` to refuse plaintext streaming for encrypted objects.

## Option B: Redirect to Crypt4GH object endpoint

- Some deployments already have a Crypt4GH service that serves re-encrypted objects by object ID (e.g. `GET /ga4gh/crypt4gh/v1/objects/{object_id}`).
- For encrypted DRS objects, you can set the DRS access URL to that Crypt4GH URL. The client then uses the same DRS object ID there; the Crypt4GH service fetches/decrypts and re-encrypts.

## Summary

- **Encrypted objects** (`storage_references.is_encrypted = true`): the bytes at `storage_key` are Crypt4GH-encrypted with the service key. To serve them with re-encryption:
  - Either expose a **stream** route that decrypts and then wrap it with **Crypt4GHLayer** (Option A), or
  - Point the DRS access URL at an existing **Crypt4GH object endpoint** (Option B).
- **Crypt4GHLayer** (in `ferrum-crypt4gh`) re-encrypts response bodies when the request has a valid passport and `X-Crypt4GH-Public-Key`. The inner service must return the **decrypted** stream for encrypted objects so the layer can re-encrypt it to the client’s key.
