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

- **Authentication:** JWT and/or GA4GH Passports; JWKS and optional issuer validation. See [INSTALLATION.md](docs/INSTALLATION.md) and [GA4GH.md](docs/GA4GH.md).
- **Encryption:** All data at rest is encrypted with Crypt4GH; downloads are re-encrypted per requester. See [CRYPT4GH.md](docs/CRYPT4GH.md) for the threat model and invariants.
- **Authorization:** Passport Visa claims and optional role-based checks before granting access to DRS objects, WES runs, workspaces, and other resources.

## OWASP alignment

We apply OWASP Top 10–oriented practices across the stack:

- **A01 Broken Access Control:** Auth middleware on requests; workspace membership and WES run visibility enforced; WES cache stats require authentication.
- **A02 Cryptographic Failures:** JWT algorithm pinning (RS256/ES256 for Passport; no `none` or algorithm confusion).
- **A03 Injection:** Input validation for DRS names, workspace names/slugs, and invite emails (length, charset, no control chars); workflow params passed via file in Nextflow (no CLI/env injection); path sanitization (`safe_join`) and SSRF-safe URL validation before fetches.
- **A07 Identification and Authentication Failures:** Token revocation (optional); optional token age limit.
- **A09 Security Logging:** Security events (access denied, auth failure, path traversal, SSRF attempts) logged and optionally persisted.
- **A10 Server-Side Request Forgery (SSRF):** URL validation policy (scheme, blocked hosts, private IPs); safe HTTP client used for outbound requests.

Security-focused tests live in the `ferrum-security-tests` crate (SSRF, validation, etc.).

---

## Known security considerations for operators

- **Secrets:** Store database URLs, S3 credentials, and Crypt4GH node keys in secret managers or restricted config; avoid committing them to version control.
- **TLS:** Use HTTPS in production; terminate TLS at a reverse proxy (e.g. nginx) or load balancer.
- **Network:** Restrict access to Ferrum and backing services (PostgreSQL, MinIO) to trusted networks where possible.
- **Updates:** Apply security and dependency updates promptly; run `cargo audit` (or similar) as part of your process.

### Production hardening checklist (non-exhaustive)

- **Identity & auth**
  - [ ] Configure `[auth]` with `require_auth = true` for production.
  - [ ] Use a trusted OIDC provider / Passport broker and set `jwks_url` and `issuer`.
  - [ ] Disable demo‑/test users in Keycloak or other IdPs.

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
  - [ ] Define and test an incident response process (including 72‑Stunden‑Pflichten, falls anwendbar).
  - [ ] Automatisieren von Updates und regelmäßigen Dependency‑Audits in CI.

---

*[← Back to README](README.md)*
