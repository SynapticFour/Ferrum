# License Compatibility

Ferrum is licensed under Apache-2.0. Dependencies must use a licence on the allow list in `deny.toml`.

## Allowed dependency licenses

- MIT
- Apache-2.0
- BSD-2-Clause, BSD-3-Clause
- ISC
- Unicode-DFS-2016
- Zlib, OpenSSL, CC0-1.0

## Explicitly forbidden dependency licenses

GPL-2.0, GPL-3.0, AGPL-3.0, LGPL (all versions)

These are enforced automatically in CI via cargo-deny. Any PR that introduces a GPL dependency will fail CI.

## Verification

Run locally at any time:

```bash
cargo deny check licenses
```
