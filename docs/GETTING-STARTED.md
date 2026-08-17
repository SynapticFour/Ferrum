# Getting started

Clone this repository. Rust 1.91 is pinned in `rust-toolchain.toml`. Docker is required for the compose paths below.

## Zero-risk proof (no Docker)

```bash
make prove
```

Runs `cargo test --workspace --all-targets`.

## Auth-on clone path (HS256)

```bash
make eval
```

`require_auth=true` (HS256). TES remains noop unless you overlay Docker TES. This is **not** ga4gh-infra Passports. Stop with `make down-eval`.

## Demo stack (NON-PILOT)

```bash
make up
```

Auth off (`require_auth=false`), TES **noop**. HelixTest stubs on. This is the demo lifecycle, **not** a pilot.

For real local containers (still not a pilot):

```bash
make up-tes
```

Optional after the stack is up: `make seed-pilot` then `make smoke-pilot`. Stop with `make down` (keep volumes) or `make destroy` (remove volumes and project images).

Full AAI (Passport-on-DRS) is `make up-pilot-local`. Do not treat `make up` as that path.

## Installer CLI (optional)

```bash
curl -sSf https://raw.githubusercontent.com/SynapticFour/Ferrum/main/install.sh | sh
export PATH="$HOME/.ferrum/bin:$PATH"
ferrum demo start          # same NON-PILOT posture as make up
ferrum demo start --edge   # SQLite + local storage, no Docker
```

Uninstall: [INSTALLATION.md](INSTALLATION.md#uninstall). Resource notes for edge: [AFRICA-DEPLOYMENT.md](AFRICA-DEPLOYMENT.md).

## What the demo does not do

| Default | How to change it |
|---------|------------------|
| TES noop | `make up-tes` |
| `require_auth=false` | `make eval` (HS256) or `make up-pilot-local` (Passports) |
| htsget region queries (`referenceName` / `start` / `end`) | HTTP 400 — whole-object tickets only |
| WES `ferrum_backend=lsf` | Errors — LSF is not implemented |

Operator matrix: [OPERATOR-TRUST.md](OPERATOR-TRUST.md). Install variants: [INSTALLATION.md](INSTALLATION.md). TES Docker: [TES-DOCKER-BACKEND.md](TES-DOCKER-BACKEND.md).

Images: `ghcr.io/synapticfour/ferrum:edge` tracks HEAD. GitHub Release tag **v0.3.1** Docker omitted `profiles/meta`. Latest Ferrum tag is **v0.3.2**; suite consumers still pin **v0.3.1** until that join is bumped.

Next: [ARCHITECTURE.md](ARCHITECTURE.md) · [FOR-EVALUATORS.md](FOR-EVALUATORS.md) · [GA4GH.md](GA4GH.md).
