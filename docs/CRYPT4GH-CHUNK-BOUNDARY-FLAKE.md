# Crypt4GH chunk-boundary flake

`ferrum-crypt4gh` test `test_encrypt_decrypt_one_byte_over_chunk_boundary` failed once in a full-suite run with `crypt4gh: No supported encryption method`, then passed five times. This note records a later repetition, the cause that repetition showed, and the directory fix now in `crates/ferrum-crypt4gh/tests/chunk_boundaries.rs`. Assertions and library code are unchanged.

## What was run

Host toolchain `rustc 1.91.1` (the channel in `rust-toolchain.toml`). The Docker image was not used. The test binary is `crates/ferrum-crypt4gh/tests/chunk_boundaries.rs`, built once with `cargo test -p ferrum-crypt4gh --test chunk_boundaries --no-run` and then executed directly so each repetition used the same binary.

| Series | Filter | Threads | Runs | Passed | Failed |
|---|---|---|---|---|---|
| Alone | `test_encrypt_decrypt_one_byte_over_chunk_boundary --exact` | default | 50 | 50 | 0 |
| Alone | same filter | `--test-threads=1` | 50 | 50 | 0 |
| Whole binary | none | default | 20 | 19 | 1 |
| Whole binary | none | `--test-threads=1` | 20 | 20 | 0 |

The single failure was whole-binary run 13 with default parallelism. In that process:

- `test_encrypt_decrypt_one_byte_over_chunk_boundary` panicked at `roundtrip` with `crypt4gh: No supported encryption method` (cause: `No supported encryption method`).
- `test_encrypt_decrypt_exact_chunk_boundary` panicked at `assert_eq!(got, payload)`. The decoded buffer was 65537 bytes. The expected payload was 65536 bytes. The shared prefix was the same `(i % 251)` sequence; the extra decoded byte was the next byte of that sequence.
- `test_encrypt_decrypt_zero_bytes` and `test_encrypt_decrypt_large_bam_like_payload` passed in the same process.

## Shared directory, not a missing libsodium init

At the time of that failure, `roundtrip` built its directory with `temp_path("chunk")`. That function used only `SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()`. Every `roundtrip` caller used the same name, `chunk`:

- `test_encrypt_decrypt_exact_chunk_boundary` (65536 bytes)
- `test_encrypt_decrypt_one_byte_over_chunk_boundary` (65537 bytes)
- `test_encrypt_decrypt_zero_bytes`
- `test_encrypt_decrypt_large_bam_like_payload` (10 MiB)

Two of those tests that sampled the clock on the same tick shared `plain.bin`, `enc.c4gh`, and `out.bin`. Each test generated its own key. One test could encrypt the other test's plaintext, and one test could decrypt the other test's ciphertext.

The exact-chunk failure matches the first case: the decoded length is the one-byte-over payload, not the 65536-byte payload that test built. The one-byte-over failure matches the second case. Vendored `crypt4gh` `deconstruct_header_body` (`third_party/crypt4gh/src/header.rs`) returns `Crypt4GHError::NoSupportedEncryptionMethod` when `decrypt` yields an empty packet list. A ciphertext written for a different key produces that empty list. The error text is `No supported encryption method`.

`test_encrypt_zero_bytes_produces_non_empty_ciphertext` used the prefix `chunk-only-encrypt`, so it was outside this set.

A run of the single test could not collide with itself. `--test-threads=1` ran the four `roundtrip` tests one after another, so they did not share the directory. Both of those schedules passed every repetition above.

## libsodium

Vendored `crypt4gh` has two `Once` statics, both named `SODIUM_INIT`. `crypt4gh::init` (`src/lib.rs`) runs before `encrypt`. `keys::init` (`src/keys.rs`) runs before `generate_private_key`. Each calls `sodiumoxide::init()`, which is safe to call more than once. A failed init panics with `Unable to initialize libsodium`. That panic did not appear in the failing run. The observed error is the empty-packet path above, which runs only after init has already returned. A missing one-time init does not explain this failure.

## Fix

`temp_path` in `chunk_boundaries.rs` now takes the process id and an atomic counter. Each call increments the counter, so the four `roundtrip` tests and `test_encrypt_zero_bytes_produces_non_empty_ciphertext` each get a distinct directory in one process. The path is `ferrum-c4gh-{name}-{pid}-{n}` under the process temporary directory. The assertions are the same. The two `SODIUM_INIT` statics are still separate. Both paths call `sodiumoxide::init`, and the failure reproduced above was a shared directory.

`rewrap.rs`, `mac_validation.rs`, and `interop.rs` still build a path from the clock. Each of those binaries calls `temp_path` once, with its own name.

After that change, the same host toolchain `rustc 1.91.1` built `chunk_boundaries` once with `cargo test -p ferrum-crypt4gh --test chunk_boundaries --no-run`. The binary then ran 40 times with default parallelism and no filter.

| Series | Filter | Threads | Runs | Passed | Failed |
|---|---|---|---|---|---|
| Whole binary after the directory fix | none | default | 40 | 40 | 0 |
