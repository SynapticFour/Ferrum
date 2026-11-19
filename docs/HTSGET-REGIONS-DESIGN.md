# Design: htsget regions with byte ranges, including Crypt4GH objects

Status: **proposal**, not implemented. Until the acceptance tests in section 7 pass, [HTSGET.md](HTSGET.md) and service-info keep saying that region slicing is not supported.

## 1. Problem

Today every htsget ticket points at the whole object. `reject_unsupported_slice` in `crates/ferrum-htsget/src/handlers.rs` answers HTTP 400 to `referenceName`, `start`, `end`, POST `regions` and `class=header`. A client asking for one gene in a 100 GB BAM downloads 100 GB.

Two further gaps sit underneath:

- **DRS `/stream` reads before it skips.** HTTP `Range` on an unencrypted object reads and discards every byte before `start` (`crates/ferrum-drs/src/handlers.rs`). A range near the end of a large file costs a full read from storage.
- **Crypt4GH objects reject `Range`.** `/stream` answers 400 when the object is encrypted, so no partial access to encrypted data exists at all.

Region tickets need both fixed: a ticket is a list of byte ranges, and the server must serve each range without reading the whole object, for plain and for encrypted objects.

## 2. Goals and non-goals

Goals:

1. htsget 1.3 region requests for **BAM** (BAI or CSI index) and **VCF.gz / BCF** (TBI or CSI index): GET with `referenceName`/`start`/`end`, POST with several `regions`, `referenceName=*` for unplaced reads, and `class=header`.
2. The same for objects stored as **Crypt4GH**, in both delivery modes Ferrum already has: plaintext over TLS (`/stream`) and Crypt4GH on the wire for the requester's key.
3. Ranged reads in storage, so cost scales with the bytes requested, not with the object.

Non-goals for the first version: CRAM (needs CRAI and reference-dependent slicing; follow-up), `fields`/`tags` filtering (the spec allows returning all fields), server-side record trimming (the spec allows a superset of records at block boundaries; clients filter).

## 3. Storage: real ranged reads

Add one method to `ObjectStorage` (`crates/ferrum-storage/src/lib.rs`):

```rust
/// Bytes [start, end] inclusive. Default: get() and skip (today's behaviour).
async fn get_range(&self, key: &str, start: u64, end: u64)
    -> Result<Box<dyn AsyncRead + Send + Unpin>>;
```

Native implementations: `local` seeks the file; `s3` sends `Range: bytes=start-end` on GetObject; `opendal` uses its range reader. The default keeps every backend working. The DRS `/stream` handler switches to `get_range` for unencrypted objects; that alone fixes the read-and-discard cost.

Synaptic-Core already has this shape (`ObjectStorage::get` with `Option<ByteRange>` in `synaptic-core-storage`). Using the same signature keeps the later move of Ferrum onto Synaptic-Core mechanical.

## 4. Plain objects: tickets from the index

### 4.1 Finding the index

An index is a DRS object of its own. Lookup order: an explicit link in object metadata (`index_object_id`); otherwise a sibling with the same name plus `.bai`, `.csi` or `.tbi` in the same dataset. No index: region requests answer `NotFound` with "no index for this object"; whole-object tickets keep working. Ingest gains an optional check that warns when a BAM or VCF.gz arrives without an index.

Indexes are small; the gateway caches parsed indexes keyed by object id and checksum.

### 4.2 Computing byte ranges

Parsing uses the **noodles** crates (MIT; `noodles-bgzf` is already in `Cargo.lock`): `noodles-bam` + `noodles-csi` for BAI/CSI, `noodles-tabix` for TBI.

For each requested region:

1. Resolve `referenceName` against the header's reference sequences (BAM) or the index's names (tabix). Unknown name: `NotFound`.
2. Query the index for chunks overlapping `[start, end)` (0-based, half-open per htsget). Each chunk is a pair of BGZF virtual offsets.
3. Turn virtual offsets into **compressed byte ranges**: the upper 48 bits are the block's file offset. The range end is the start of the block after the chunk's end block; reading the block header there gives its size, or the next chunk start is used.
4. Merge overlapping and adjacent ranges across all regions of the request.

### 4.3 Ticket layout

```json
{ "htsget": { "format": "BAM", "urls": [
  { "url": "https://…/drs/v1/objects/ID/stream", "headers": { "Range": "bytes=0-<header end>" }, "class": "header" },
  { "url": "https://…/drs/v1/objects/ID/stream", "headers": { "Range": "bytes=a-b" }, "class": "body" },
  { "url": "data:application/octet-stream;base64,<BGZF EOF block>", "class": "body" }
] } }
```

- **Header**: bytes from 0 to the first alignment block (BAM: end of the BGZF blocks holding the header; VCF: the `#` lines' blocks). `class=header` returns only this URL plus the EOF block.
- **Body**: one URL per merged range, each with a `Range` header on the existing `/stream` endpoint. Authorization is the same Bearer as today.
- **EOF**: the 28-byte BGZF EOF marker as a `data:` URL, so the concatenation is a valid BGZF file.
- `referenceName=*` (unplaced reads) uses the index's unplaced-reads pseudo-bin / the offset after the last placed chunk.

Concatenating the URLs in order gives a valid BAM/VCF.gz that `samtools view` / `bcftools view` reads. It may contain records just outside the region; that is allowed by the spec.

## 5. Crypt4GH objects

### 5.1 Layout facts

A Crypt4GH file is a header, then the plaintext cut into **65 536-byte segments**, each stored as nonce (12) + ciphertext + MAC (16) = **65 564 bytes**. Plaintext offset `p` lives in segment `p / 65536`, which starts at ciphertext offset `header_len + (p / 65536) * 65564`. The index (section 4) is computed on the **plaintext**, so it gives plaintext ranges; this mapping turns them into ciphertext ranges.

Indexes are stored unencrypted, or decrypted server-side with the node key; they hold positions, not sequence data.

### 5.2 Mode A: plaintext over TLS (`/stream`)

For plaintext range `[a, b]`:

1. Segments `s0 = a / 65536` to `s1 = b / 65536`.
2. `get_range(header)` once (cached per object), then `get_range` of segments `s0..=s1` only.
3. Decrypt those segments with the session key from the header, drop `a - s0*65536` bytes at the front and trim the end at `b`.

This lifts the "Range not supported for Crypt4GH" restriction in `/stream`, so section 4's tickets work unchanged for encrypted objects in this mode. The vendored crypt4gh already has the pieces (`body_decrypt_parts`, segment constants); what is new is reading only the needed segments instead of streaming from 0.

### 5.3 Mode B: Crypt4GH on the wire for the requester

The client must receive a valid Crypt4GH stream for **its** key that decrypts to the region only. The Crypt4GH spec has a header packet for exactly this: the **data edit list** (packet type 1), alternating "skip n bytes, keep m bytes" over the decrypted segments.

Ticket for an encrypted object in this mode:

1. **Header URL**: a new endpoint `GET /ga4gh/drs/v1/objects/{id}/crypt4gh-header?ranges=a-b,c-d` with `X-Crypt4GH-Public-Key`. It decrypts the stored header with the node key, re-encrypts the data-encryption packet for the requester (as re-wrap does today) and adds one **edit list packet** built from the requested plaintext ranges relative to the segments being sent.
2. **Body URLs**: the ciphertext segment ranges from 5.1, served raw (no decryption) via `/stream` with a new `?raw=1` for encrypted objects, which today's re-wrap path already effectively does for the whole body.

The client concatenates and decrypts with any Crypt4GH tool; the edit list makes the output exactly the requested bytes. In this mode the header and EOF cannot be separate plaintext `data:` URLs, so the plaintext ranges always include the file's header bytes (from 0) and its own last 28 bytes (the BGZF EOF block); the decrypted output is then a valid file. The vendored crypt4gh has `make_packet_data_edit_list` and a `rearrange` path for a single range; multiple ranges need the edit list built from all of them (and `rearrange` today calls `unimplemented!()` when an edit list already exists, which stored objects never have).

Reference implementation to compare against: [htsget-rs](https://github.com/umccr/htsget-rs) implements Crypt4GH region tickets the same way (edit list plus raw segments).

## 6. Where the code lives

The Crypt4GH segment mapping and edit-list building are the same work as `sc-crypt4gh-streaming` on the Synaptic-Core roadmap ("true streaming Crypt4GH + header rewrap"). Ferrum is to become Synaptic-Core plus a GA4GH adapter. To build this once:

- Pure, I/O-free functions (plaintext range → segment range, edit list from ranges, header re-wrap with edit list) go into **one small shared crate**, Apache-2.0, with property tests. Ferrum and `synaptic-core-crypto` both depend on it.
- Index parsing and ticket building stay in `ferrum-htsget` (GA4GH-specific).
- `get_range` goes into both storage traits (Synaptic-Core has it already).

Owner decision needed: shared crate now, or Ferrum first and extract later.

## 7. Acceptance tests (before the docs change)

| Test | Data | Pass when |
|------|------|-----------|
| Range mapping (unit, property) | synthetic | every plaintext range maps to the minimal segment set; edit list round-trips |
| BAM region, plain | GIAB HG002 chr20 BAM + BAI (public) | records from concatenated ticket = `samtools view in.bam chr20:a-b` (after filtering to the region) |
| VCF region, plain | GIAB HG002 v4.2.1 VCF.gz + TBI | same with `bcftools view -r` |
| Multi-region POST, `*`, `class=header` | same | header-only output parses; unplaced reads present for `*` |
| BAM region, Crypt4GH mode A | same BAM, ingested encrypted | identical bytes to the plain case |
| BAM region, Crypt4GH mode B | same | `crypt4gh decrypt` of the concatenation equals mode A output |
| Cost | 30 GB BAM | bytes read from storage ≤ 2 × bytes in the ticket |
| Conformance | HelixTest htsget suite | region tests pass without stubs |

The data are the same public GIAB files the gatk-rs test bed puts behind DRS, so one dataset serves both projects.

## 8. Steps

1. `get_range` in storage + `/stream` using it (fixes plain Range cost). Small, independent.
2. Plain BAM/VCF region tickets + `class=header` (sections 4.1–4.3).
3. Shared Crypt4GH range crate + mode A.
4. Mode B header endpoint + tickets.
5. Docs, service-info, HelixTest region tests on; remove "not supported" only here.
