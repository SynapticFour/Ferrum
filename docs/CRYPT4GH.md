# Crypt4GH: Transparent Encryption

> Ferrum stores all data encrypted at rest using Crypt4GH and re-encrypts each download specifically for the requester — without ever storing plaintext on disk or transmitting it over the wire.

This document describes how Crypt4GH works, Ferrum’s header re-wrapping extension, security invariants, key exchange, key management, and client usage.

---

## How Crypt4GH works (standard)

The [Crypt4GH](https://github.com/elixir-oslo/crypt4gh) format defines:

- A **header** (encrypted with one or more recipient public keys) containing session keys and segment info.
- **Data segments** encrypted with ChaCha20-Poly1305 using keys derived from the header.

Key exchange uses **X25519**; encryption uses **ChaCha20-Poly1305**. Recipients decrypt the header with their private key to obtain the session key, then decrypt the segments.

```mermaid
flowchart LR
  subgraph File["Crypt4GH file"]
    H[Encrypted header]
    S1[Segment 1]
    S2[Segment 2]
    S3[...]
  end
  H --> S1 --> S2 --> S3
```

---

## Ferrum’s extension — header re-wrapping

**Problem:** Standard Crypt4GH requires knowing all recipients at ingest time. In a multi-tenant or Passport-based system, recipients are not known in advance.

**Solution:** At ingest, Ferrum encrypts objects with a **node master key** as the sole recipient. On download, after authorization, Ferrum **re-wraps only the header** for the requester’s public key. The body is never re-encrypted.

- **Ingest:** Client (or pipeline) sends plaintext or already-encrypted data; Ferrum encrypts with the node key and stores header + body.
- **Download:** Client sends auth (e.g. Passport) and `X-Crypt4GH-Public-Key`. Ferrum decrypts the header with the node key, re-encrypts the header for the client’s key, and streams **new header + same body** to the client.

```mermaid
sequenceDiagram
  participant Client
  participant DRS
  participant Crypt4GH as Crypt4GH Layer
  participant Storage

  Client->>DRS: GET /objects/{id}/access (Auth + X-Crypt4GH-Public-Key)
  DRS->>DRS: Authorize
  DRS->>Crypt4GH: open_stream(object_id, client_pubkey)
  Crypt4GH->>Storage: get(object_id)
  Storage-->>Crypt4GH: Encrypted stream (node key)
  Crypt4GH->>Crypt4GH: Decrypt header (node key)
  Crypt4GH->>Crypt4GH: Re-encrypt header (client key)
  Crypt4GH-->>DRS: Stream: new header + same body
  DRS-->>Client: 200 body
```

> **O(1) re-encryption** — The Crypt4GH header is typically &lt; 1 KB. Re-wrapping a 500 GB BAM file takes the same time as re-wrapping a 1 KB text file. The body stream passes through with zero-copy semantics.

---

## Security invariants

1. **Zero plaintext at rest** — Stored objects are always encrypted under the node key (or a key derived from it).
2. **Zero plaintext in transit** — The server never sends decrypted body; it sends a stream encrypted for the client’s key.
3. **Per-requester encryption** — Each download gets a header encrypted for that requester’s public key.
4. **Authorization before decryption** — Access control (e.g. Passport/Visa) is enforced before any header decryption or re-wrap.
5. **Node key isolation** — The node private key is never exposed via the API; it is used only inside the Crypt4GH layer for header decryption and re-wrap.

---

## Key exchange protocol (random access)

For tools that need random access (e.g. `samtools`, `htseq`, `tabix`), Ferrum supports a key exchange so the client can perform decryption locally while the server streams the body.

1. Client sends a request to `/ga4gh/crypt4gh/v1/keys/exchange` with auth and temporary public key.
2. Server returns a **wrapped session key** (encrypted for the client’s key) and optional range/segment info.
3. Client decrypts the session key and uses it to decrypt the stream (or segments) received from the DRS access endpoint.

Example (conceptual):

```bash
# Request wrapped key for object (authenticated)
curl -s -H "Authorization: Bearer $TOKEN" \
  -H "X-Crypt4GH-Public-Key: $(base64 -w0 < client.pub)" \
  "https://ferrum.example.com/ga4gh/crypt4gh/v1/keys/exchange?object_id=$ID" \
  -o wrapped_key.bin
```

---

## Key management

| Store | Use case | Notes |
|-------|----------|--------|
| **LocalKeyStore** | Single-node, file-based | Keys in `/etc/ferrum/keys/` or config path |
| **DatabaseKeyStore** | Multi-node, shared keys | Keys stored encrypted in DB (optional) |
| **VaultKeyStore** | HPC / enterprise | HashiCorp Vault for key storage (optional) |

**Generate node keypair:**

```bash
ferrum keys generate
# Writes to config path, e.g. /etc/ferrum/keys/node.key, node.key.pub
```

**Rotate keys:** Run `ferrum keys rotate` to re-encrypt all object headers under a new node key. This touches only headers (metadata), not body segments; runtime depends on number of objects, not total data size.

---

## Client usage

**Download (already encrypted for your key):**

```bash
# Download
curl -H "Authorization: Bearer $PASSPORT" \
     -H "X-Crypt4GH-Public-Key: $(cat ~/.ssh/key.pub.b64)" \
     "https://ferrum.institution.edu/ga4gh/drs/v1/objects/$ID/access/https" \
     -o data.c4gh

# Decrypt with any Crypt4GH client
crypt4gh decrypt --sk ~/.ssh/key.c4gh < data.c4gh > data.bam
```

`key.pub.b64` is your public key in base64 (as required by the header format). The server re-wraps the header for this key; the response is a valid Crypt4GH file that only your private key can decrypt.

---

## Compatibility

Ferrum’s Crypt4GH layer is compliant with the **Crypt4GH v1** specification. Compatible clients include:

- [ga4gh/crypt4gh](https://github.com/elixir-oslo/crypt4gh) (reference implementation)
- [crypt4gh-rust](https://crates.io/crates/crypt4gh) (Rust)
- [crypt4gh](https://pypi.org/project/crypt4gh/) (Python)

---

*[← Documentation index](README.md)*
