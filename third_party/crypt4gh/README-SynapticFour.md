# crypt4gh (vendored, EGA-archive main)

Vendored from [EGA-archive/crypt4gh-rust](https://github.com/EGA-archive/crypt4gh-rust) at commit
`2d41a17` (2024-02-01, "Merge pull request #3 from umccr/fix_rust_crypto_et_al"). Crate version
string `0.4.1`. Apache-2.0; the license text in `LICENSE` is upstream's, unchanged. Upstream
statements are listed in [NOTICE](NOTICE). Two Synaptic Four changes:

- Log lines that printed secret key material are removed (see NOTICE).
- The unused dependency `ed25519_to_curve25519` (GPL-3.0; only named in a code comment) is removed
  from `Cargo.toml`, so the vendored crate's dependency tree has no copyleft licence. `cargo deny
  check licenses` enforces this.

## Why this version

- The crates.io release `crypt4gh` 0.4.1 depends on `rust-crypto` (RUSTSEC-2022-0011, critical)
  and `sodiumoxide` (deprecated, libsodium C library).
- Upstream `main` since PR #3 (contributed by UMCCR, the htsget-rs authors) uses RustCrypto only:
  `chacha20poly1305`, `crypto_kx`, `curve25519-dalek`, `scrypt`, `bcrypt-pbkdf`, `aes`, `ctr`, `cbc`.
- htsget-rs (`htsget-config` / `htsget-storage` with the `experimental` feature, used by
  `ferrum-htsget` for Crypt4GH region tickets) is written against this API
  (`get_private_key(PathBuf, Result<String, _>)`, `get_public_key(PathBuf)`).

The earlier Synaptic Four fork (crates.io 0.4.1 with `rust-crypto` replaced, libsodium kept) is
superseded by this tree. The Crypt4GH file format is the same, so existing keys and encrypted
objects stay readable; `crates/ferrum-crypt4gh/tests` and `crates/ferrum-htsget/tests` cover
round trips, re-wrapping, ranges and region tickets.

Wired via `[patch.crates-io]` in the Ferrum workspace root `Cargo.toml`. This directory has no
`Cargo.lock`; the workspace lockfile is the only one.
