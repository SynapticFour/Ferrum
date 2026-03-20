# Changelog

All notable changes to this project will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Initial implementation of all GA4GH services (DRS, WES, TES, TRS, Beacon v2, Passports).
- Transparent Crypt4GH encryption layer with header re-wrapping (O(1) per download).
- WES support for Nextflow, CWL, WDL, Snakemake.
- Beacon v2 with three access tiers.
- Single-command demo deployment (Docker Compose, Makefile, init script).
- Helm chart and Ansible playbooks for production and HPC.
- GitHub Actions CI (test, clippy) and release workflows (multi-arch binaries).
- Install script (`install.sh`) for macOS and Linux.
- Documentation: README, ARCHITECTURE, INSTALLATION, CRYPT4GH, GA4GH, WORKFLOWS, CONTRIBUTING, SECURITY.
- htsget 1.3.0 ticket/stream integration (tickets refer to DRS `/stream` URLs).

### Fixed
- htsget routing reliability: compose router/state so ticket endpoints don’t 404 with empty bodies (HelixTest htsget suite).
- CI reliability: build the gateway using an official mirror (ECR public) and retry gateway Docker builds when registries are temporarily flaky.

---

*[← Back to README](README.md)*
