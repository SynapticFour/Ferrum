# Security policy

## Supported versions

| Version | Supported          |
|---------|--------------------|
| Latest major (e.g. 0.x) | ✅ Yes |
| Older majors / minors | ❌ Best effort only |

We recommend always running the latest patch release of the current major version.

---

## Reporting vulnerabilities

We take security seriously. Please report vulnerabilities **privately** to avoid putting users at risk.

1. **Do not** open a public GitHub issue for a security vulnerability.
2. Email the maintainers (see repository contacts or organization page) or use GitHub Security Advisories if available: **Security** tab → **Report a vulnerability**.
3. Include a description, steps to reproduce, and impact. We will acknowledge and work with you on a fix and disclosure timeline.
4. After a fix is released, we may publish an advisory and credit you unless you prefer to remain anonymous.

---

## Security model overview

- **Threat model (adversaries, assets, residual risk):** [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)
- **Incident response (product runbook):** [docs/INCIDENT_RESPONSE.md](docs/INCIDENT_RESPONSE.md)
- **Authentication:** JWT and/or GA4GH Passports; JWKS, optional issuer and **audience**. Code default `require_auth=true`. See [OPERATOR-TRUST.md](docs/OPERATOR-TRUST.md), [INSTALLATION.md](docs/INSTALLATION.md) and [GA4GH.md](docs/GA4GH.md).
- **Encryption:** Crypt4GH uses **ChaCha20-Poly1305** when at-rest encryption is enabled. TLS is the operator reverse-proxy’s responsibility. Default ingest encryption is off (`default_encrypt_upload=false`).
- **Authorization:** Exact Passport visa matches (not substring); admin routes stay gated. Client `X-ADS-Base-URL` is ignored.
- **Compute:** TES Docker request volumes are prefix-allowlisted. WES LSF is not implemented. Slurm interpolates POSIX-quoted paths only.
- **Supply chain:** `cargo deny check` in CI (`deny.toml`); SBOM on release.

## OWASP alignment

We apply OWASP Top 10–oriented practices across the stack:

- **A01 Broken Access Control:** Auth middleware; exact visa roles; admin APIs always gated; workspace membership and WES run visibility enforced.
- **A02 Cryptographic Failures:** JWT algorithm pinning (RS256/ES256 for Passport; no `none` or algorithm confusion). Crypt4GH ChaCha20-Poly1305 when enabled.
- **A03 Injection:** Input validation; Slurm POSIX quoting of workflow URLs; TES volume allowlist (no arbitrary host binds).
- **A07 Identification and Authentication Failures:** Fail-closed `require_auth` default; optional audience; token revocation (optional).
- **A09 Security Logging:** Security events logged; residency audit append failures are warned, not dropped.
- **A10 Server-Side Request Forgery (SSRF):** URL validation (scheme, blocked hosts, private IPv4/IPv6 ULA/link-local/mapped, DNS resolve); client ADS URL override ignored.

Security-focused tests live in the `ferrum-security-tests` crate (SSRF, validation, etc.).

---

## Known security considerations for operators

- **Secrets:** Store database URLs, S3 credentials, and Crypt4GH node keys in secret managers or restricted config; avoid committing them to version control.
- **TLS:** Use HTTPS in production; terminate TLS at a reverse proxy (e.g. nginx) or load balancer.
- **Network:** Restrict access to Ferrum and backing services (PostgreSQL, MinIO) to trusted networks where possible.
- **Updates:** Apply security and dependency updates promptly; CI runs **cargo-deny** (`deny.toml`). Operators may additionally run `cargo audit`. Org policy: [DEPENDENCY-UPDATE-POLICY.md](https://github.com/SynapticFour/synapticfour-infra/blob/main/docs/DEPENDENCY-UPDATE-POLICY.md).
- **MII manifest sync (`ferrum mii sync-manifest`):** When not using `--offline`, the CLI downloads FHIR NPM packages over HTTPS from the configured registry (default `https://packages.fhir.org`). Treat downloaded `.tgz` files like any other supply-chain artifact: verify versions against your change-management policy, prefer offline mirrors in high-assurance environments, and store caches with appropriate access controls.

### Production hardening checklist (non-exhaustive)

- **Identity & auth**
  - [ ] Configure `[auth]` with `require_auth = true` for production (code default; do not ship demo compose).
  - [ ] Use a trusted OIDC provider / Passport broker and set `jwks_url`, `issuer`, and `audience` if your tokens carry `aud`.
  - [ ] Unset `FERRUM_TES_HELIXTEST_STUB` and `FERRUM_WES_HELIXTEST_STUBS`.
  - [ ] Set a tight `FERRUM_TES_ALLOWED_BIND_PREFIXES` if TES Docker is enabled.

- **Transport security**
  - [ ] Terminate TLS with modern ciphers and protocols; prefer TLS 1.2+.
  - [ ] Enforce HTTPS (HSTS) at the reverse proxy or load balancer.

- **Data at rest**
  - [ ] Configure Crypt4GH node keys with restrictive file permissions.
  - [ ] Ensure backups of PostgreSQL and object storage are encrypted and access‑controlled.

- **Access control & logging**
  - [ ] Enable and monitor security/audit logs; forward to a central log system.
  - [ ] Periodically review access rules (datasets, workspaces, Passport policies).

- **Operations**
  - [x] Define an incident response process — see [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md) (test via tabletop; company plan in synapticfour-business).
  - [x] Automate dependency policy checks in CI (`cargo deny check`).

---

*[← Back to README](README.md)*
