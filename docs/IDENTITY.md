# Who Ferrum is for

Ferrum is a **GA4GH data/compute platform** for institutes that must keep genomic data on their own hardware.

It is a complete product without Solum, BRA, or ga4gh-infra. Built-in passports cover standalone auth. Optional joins add Passports-from-infra and consent-from-Solum — they do not replace this repo.

## Audience

Genomics core facilities, archive nodes, field labs that need DRS / WES / TES / TRS / Beacon / htsget / Crypt4GH behind one gateway.

**Not for:** hospital EHR workflows (that is Solum), literature/Phenopacket workbenches (that is BRA), running an AAI broker (that is ga4gh-infra).

## Standalone

```bash
git clone https://github.com/SynapticFour/Ferrum.git && cd Ferrum
make up          # local demo stack
```

Proof that the APIs behave: clone [HelixTest](https://github.com/SynapticFour/HelixTest) and run `helixtest --all --mode ferrum` against that stack. HelixTest is not GA4GH certification.

## Optional composition (more than the sum)

| Join | What you gain | Contract |
|------|----------------|----------|
| ga4gh-infra | External AAI, visas, ADS | Ferrum `[auth] mode = "external"`; pin in `VERSIONS.lock` |
| Solum | Consent-gated DRS/WES | Ferrum `[solum]` HTTP client, fail-closed |
| HelixTest | Independent conformance | Separate repo, Apache-2.0 |
| HELIOS | Signed pipeline/audit evidence | File ingest, not an API fabric |
| BRA | Researcher UI | BRA talks to Solum/its own WES; it does not embed Ferrum |

See [ECOSYSTEM.md](ECOSYSTEM.md) for ports and lifecycle verbs.
