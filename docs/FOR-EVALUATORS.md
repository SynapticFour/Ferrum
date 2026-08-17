# For evaluators

Factual snapshot of this repository. Not a sales brief. Not legal advice.

## Maturity

**Active (beta).** One human maintains this tree ([GOVERNANCE.md](GOVERNANCE.md)): no second committer with merge rights, no code escrow. There is **no** third-party security audit. HelixTest CI is **not** GA4GH certification.

Latest git tag on this repo: **v0.3.2**. Suite consumers still pin git tag **v0.3.1** until that join is bumped. See [SUITE-OVERVIEW](SUITE-OVERVIEW.md).

## License

Business Source License 1.1. Additional Use Grant: non-commercial research, academic, and internal research use. Change License Apache-2.0 four years after each version. See [LICENSE](../LICENSE) and [BUSINESS-MODEL.md](BUSINESS-MODEL.md).

## Tested in this tree

| Claim | Evidence |
|-------|----------|
| Workspace tests | `make prove` (`cargo test --workspace --all-targets`) in CI |
| Demo HelixTest | Conformance workflow, **NON-PILOT**: auth off, TES noop, stubs on |
| Docker TES | `test-tes` CI job / `make up-tes` — not the default demo |
| cargo-deny | `deny.toml` in CI |
| SBOM | Generated on release (`cargo cyclonedx`); see [RELEASING.md](../RELEASING.md) |

## Not tested / not implemented / not claimed

| Topic | Status |
|-------|--------|
| GA4GH certification | Not claimed. HelixTest is a technical signal against published OpenAPI. |
| Third-party audit | None. |
| htsget genomic region slicing | HTTP 400. Tickets are whole-object DRS streams. |
| TES default demo compute | Noop (API lifecycle only). |
| LSF | `ferrum_backend=lsf` errors. Slurm exists; LSF does not. |
| Helm as “production-ready” | Charts exist; operator still owns IdP, secrets, and network. |
| Combo SKU with Solum / BRA / infra | Does not exist. |

Fail-closed notes (unless demo flags are set): JWT/Passport required when `require_auth` is on; admin APIs always require an admin visa; TES bind-mounts require an operator prefix allowlist. Matrix: [OPERATOR-TRUST.md](OPERATOR-TRUST.md).

## Contact

Questions can be sent to [contact@synapticfour.com](mailto:contact@synapticfour.com).
